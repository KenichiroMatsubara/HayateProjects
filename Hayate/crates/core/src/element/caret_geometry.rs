//! `Caret Geometry` — text-input の幾何依存 edit 操作が問い合わせる純粋 query seam
//! （ADR-0122 決定 5）。
//!
//! 縦移動 ↑↓・表示行 Home/End・point→byte の hit は、解決済みの行レイアウトに対する
//! 6 つの query（[`CaretGeometry`]）に縮約できる。`EditState` の操作にこの seam を
//! **注入**することで、移動解決を full tree（`commit_frame` / `render`）なしに純粋に
//! テストできる。
//!
//! seam の裏には 2 つの adapter が居る（2 adapter = 本物の seam）：
//!
//! - [`ParleyCaretGeometry`]：実レイアウト（`content_layout` の Parley `Layout`）を包む。
//! - [`TableCaretGeometry`]：行→byte 範囲・byte→x を手書きした test adapter。Parley を
//!   立てずに `byte_at_x_on_line` / `byte_at_point` 等の幾何挙動を単体検証する。
//!
//! `Taffy Projection`（block-box レイアウト）と直交する text 側の query であり、
//! goal column（`edit.desired_x`）は seam ではなく `EditState` 側に残る。

use crate::element::text::TextBrush;

/// 行ヒットテストでキャレットを落とす y を、対象行の上端（`block_max_coord`）から
/// `ascent` 何割ぶん戻すか。Parley 自身の行ステップ（ベースライン近傍）を写し、
/// `byte_at_x_on_line` がその行の本文に当たるようにする。
const LINE_HIT_ASCENT_FRACTION: f64 = 0.5;

/// text-input の解決済み行レイアウトに対する純粋 query seam（ADR-0122 決定 5）。
///
/// すべて読み取り専用で、`EditState` の幾何依存操作（縦移動・表示行 Home/End・
/// point→byte）はこの 6 query だけに依存する。実装は Parley を包む実 adapter と
/// 手書きテーブルの test adapter の 2 つ。
pub trait CaretGeometry {
    /// 表示行（ソフトラップ後）の総数。
    fn line_count(&self) -> usize;

    /// バイト `byte` のキャレットが載る表示行のインデックス。ソフトラップ境界は
    /// downstream（後続行の先頭）に倒す。
    fn line_of(&self, byte: usize) -> usize;

    /// バイト `byte` のキャレットの content-local な x 座標（ゴール列の素材）。
    fn x_of(&self, byte: usize) -> f32;

    /// 表示行 `line` 上で x 座標 `x` をヒットテストしたときに着地するバイトオフセット。
    /// 縦移動がゴール列を隣接行へ写すときの中核 query。
    fn byte_at_x_on_line(&self, line: usize, x: f32) -> usize;

    /// 表示行 `line` の `[start, end)` バイト範囲（表示行 Home/End の素材）。末尾の
    /// 改行/空白の扱いは consumer 側に委ねる素の範囲を返す。
    fn line_bounds(&self, line: usize) -> (usize, usize);

    /// content-local 点 `(x, y)` をヒットテストして得るバイトオフセット（drag-select /
    /// クリックキャレット配置の素材）。
    fn byte_at_point(&self, x: f32, y: f32) -> usize;
}

/// 実レイアウト（Parley `Layout`）を包む `Caret Geometry` の実 adapter
/// （ADR-0122 決定 5）。`content_layout.layout` を借りて 6 query を提供する。
/// 既存の `caret_display_line` / `Cursor::from_point` と同一のヒット意味論を持つ。
pub struct ParleyCaretGeometry<'a> {
    layout: &'a parley::Layout<TextBrush>,
}

impl<'a> ParleyCaretGeometry<'a> {
    /// 解決済みレイアウトを借りて adapter を作る。
    pub fn new(layout: &'a parley::Layout<TextBrush>) -> Self {
        Self { layout }
    }

    /// キャレットの表示行とその content-local x を一度に求める（`caret_display_line`
    /// と同形）。downstream affinity で境界バイトを後続行に倒す。
    fn caret_line_and_x(&self, byte: usize) -> (usize, f32) {
        use parley::{Affinity, Cursor};
        let g = Cursor::from_byte_index(self.layout, byte, Affinity::Downstream)
            .geometry(self.layout, 0.0);
        let caret_x = g.x0 as f32;
        let caret_mid_y = (g.y0 + g.y1) / 2.0;
        let line_count = self.layout.len();
        let line = (0..line_count)
            .find(|&i| {
                self.layout.get(i).is_some_and(|line| {
                    let m = line.metrics();
                    caret_mid_y >= m.block_min_coord as f64 && caret_mid_y < m.block_max_coord as f64
                })
            })
            .unwrap_or_else(|| line_count.saturating_sub(1));
        (line, caret_x)
    }

    /// 表示行 `line` のヒットテスト用 y（行内のベースライン近傍）。Parley 自身の
    /// 行ステップを写す。
    fn hit_y_for_line(&self, line: usize) -> Option<f32> {
        let l = self.layout.get(line)?;
        let m = l.metrics();
        Some((m.block_max_coord as f64 - m.ascent as f64 * LINE_HIT_ASCENT_FRACTION) as f32)
    }
}

impl CaretGeometry for ParleyCaretGeometry<'_> {
    fn line_count(&self) -> usize {
        self.layout.len()
    }

    fn line_of(&self, byte: usize) -> usize {
        self.caret_line_and_x(byte).0
    }

    fn x_of(&self, byte: usize) -> f32 {
        self.caret_line_and_x(byte).1
    }

    fn byte_at_x_on_line(&self, line: usize, x: f32) -> usize {
        use parley::Cursor;
        let clamped = line.min(self.layout.len().saturating_sub(1));
        let Some(y) = self.hit_y_for_line(clamped) else {
            return 0;
        };
        Cursor::from_point(self.layout, x, y).index()
    }

    fn line_bounds(&self, line: usize) -> (usize, usize) {
        match self.layout.get(line) {
            Some(l) => {
                let r = l.text_range();
                (r.start, r.end)
            }
            None => (0, 0),
        }
    }

    fn byte_at_point(&self, x: f32, y: f32) -> usize {
        use parley::Cursor;
        Cursor::from_point(self.layout, x, y).index()
    }
}

/// 手書きテーブルの `Caret Geometry` test adapter（ADR-0122 決定 5）。各表示行を
/// `[start, end)` バイト範囲と、その行上のキャレット列 `(byte, x)` の表で表す。
/// Parley を立てずに `byte_at_x_on_line` / `byte_at_point` 等を決定的に検証できる。
pub struct TableCaretGeometry {
    lines: Vec<TableLine>,
    line_height: f32,
}

/// `TableCaretGeometry` の 1 表示行：バイト範囲とキャレット列の表。
struct TableLine {
    start: usize,
    end: usize,
    /// この行上のキャレット列 `(byte, content-local x)`。byte 昇順、両端を含む。
    columns: Vec<(usize, f32)>,
}

impl TableCaretGeometry {
    /// 行の表と行高から test adapter を作る。各行は `(start, end, columns)` で、
    /// `columns` は `(byte, x)` のキャレット列。`line_height` は `byte_at_point` の
    /// y→行 マッピングに使う。
    pub fn new(lines: Vec<(usize, usize, Vec<(usize, f32)>)>, line_height: f32) -> Self {
        Self {
            lines: lines
                .into_iter()
                .map(|(start, end, columns)| TableLine { start, end, columns })
                .collect(),
            line_height,
        }
    }

    /// `line` を有効範囲にクランプする（空テーブルなら 0）。
    fn clamp_line(&self, line: usize) -> usize {
        line.min(self.lines.len().saturating_sub(1))
    }
}

impl CaretGeometry for TableCaretGeometry {
    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line_of(&self, byte: usize) -> usize {
        // 半開区間 `[start, end)` で含む最初の行。境界バイトは後続行が `start` として
        // 拾う（downstream）。どの行の手前でもなければ最終行。
        self.lines
            .iter()
            .position(|l| byte < l.end)
            .unwrap_or_else(|| self.lines.len().saturating_sub(1))
    }

    fn x_of(&self, byte: usize) -> f32 {
        let line = self.line_of(byte);
        match self.lines.get(line) {
            Some(l) => nearest_by(&l.columns, |&(b, _)| abs_diff(b, byte))
                .map(|&(_, x)| x)
                .unwrap_or(0.0),
            None => 0.0,
        }
    }

    fn byte_at_x_on_line(&self, line: usize, x: f32) -> usize {
        let line = self.clamp_line(line);
        match self.lines.get(line) {
            Some(l) => nearest_by(&l.columns, |&(_, cx)| (cx - x).abs())
                .map(|&(b, _)| b)
                .unwrap_or(l.start),
            None => 0,
        }
    }

    fn line_bounds(&self, line: usize) -> (usize, usize) {
        match self.lines.get(self.clamp_line(line)) {
            Some(l) => (l.start, l.end),
            None => (0, 0),
        }
    }

    fn byte_at_point(&self, x: f32, y: f32) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let line = (y / self.line_height).floor();
        let line = if line < 0.0 {
            0
        } else {
            (line as usize).min(self.lines.len() - 1)
        };
        self.byte_at_x_on_line(line, x)
    }
}

/// `items` の中で `key` が最小の要素（同点は先勝ち＝小さい index）。
fn nearest_by<T, K: PartialOrd>(items: &[T], key: impl Fn(&T) -> K) -> Option<&T> {
    items.iter().reduce(|best, cur| {
        if key(cur) < key(best) {
            cur
        } else {
            best
        }
    })
}

/// `usize` の差の絶対値。
fn abs_diff(a: usize, b: usize) -> usize {
    if a >= b {
        a - b
    } else {
        b - a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3 表示行のテーブル：
    /// - 行0: `[0, 5)`  列 0→0px, 2→20px, 5→50px
    /// - 行1: `[5, 9)`  列 5→0px, 7→20px, 9→40px
    /// - 行2: `[9, 12)` 列 9→0px, 12→30px
    /// 行高 10px。ソフトラップ境界はバイト 5 と 9。
    fn table() -> TableCaretGeometry {
        TableCaretGeometry::new(
            vec![
                (0, 5, vec![(0, 0.0), (2, 20.0), (5, 50.0)]),
                (5, 9, vec![(5, 0.0), (7, 20.0), (9, 40.0)]),
                (9, 12, vec![(9, 0.0), (12, 30.0)]),
            ],
            10.0,
        )
    }

    #[test]
    fn line_count_is_number_of_display_lines() {
        assert_eq!(table().line_count(), 3);
    }

    #[test]
    fn line_of_finds_containing_line() {
        let t = table();
        assert_eq!(t.line_of(0), 0);
        assert_eq!(t.line_of(3), 0);
        assert_eq!(t.line_of(6), 1);
        assert_eq!(t.line_of(11), 2);
    }

    #[test]
    fn line_of_puts_wrap_boundary_on_following_line() {
        // 境界バイト 5 は前行の末尾でもあるが downstream で後続行（行1）に倒れる。
        let t = table();
        assert_eq!(t.line_of(5), 1);
        assert_eq!(t.line_of(9), 2);
    }

    #[test]
    fn line_of_past_end_clamps_to_last_line() {
        assert_eq!(table().line_of(12), 2);
        assert_eq!(table().line_of(99), 2);
    }

    #[test]
    fn x_of_reads_caret_column_on_its_line() {
        let t = table();
        assert_eq!(t.x_of(0), 0.0);
        assert_eq!(t.x_of(2), 20.0);
        // バイト 7 は行1（5→0, 7→20）の列。
        assert_eq!(t.x_of(7), 20.0);
    }

    #[test]
    fn byte_at_x_on_line_snaps_to_nearest_column() {
        let t = table();
        // 行1 の列は 0/20/40px（byte 5/7/9）。22px は 20px に最近 → byte 7。
        assert_eq!(t.byte_at_x_on_line(1, 22.0), 7);
        // 38px は 40px に最近 → byte 9。
        assert_eq!(t.byte_at_x_on_line(1, 38.0), 9);
        // 行0 で 48px は 50px に最近 → byte 5。
        assert_eq!(t.byte_at_x_on_line(0, 48.0), 5);
    }

    #[test]
    fn byte_at_x_on_line_clamps_line_index() {
        let t = table();
        // 範囲外の行は最終行（行2: 0→9, 30→12）にクランプ。
        assert_eq!(t.byte_at_x_on_line(99, 0.0), 9);
        assert_eq!(t.byte_at_x_on_line(99, 30.0), 12);
    }

    #[test]
    fn line_bounds_returns_byte_range() {
        let t = table();
        assert_eq!(t.line_bounds(0), (0, 5));
        assert_eq!(t.line_bounds(1), (5, 9));
        assert_eq!(t.line_bounds(2), (9, 12));
    }

    #[test]
    fn byte_at_point_maps_y_to_line_then_hits_x() {
        let t = table();
        // y=5 → 行0、x=20 → byte 2。
        assert_eq!(t.byte_at_point(20.0, 5.0), 2);
        // y=12 → 行1、x=0 → byte 5。
        assert_eq!(t.byte_at_point(0.0, 12.0), 5);
        // y=25 → 行2、x=30 → byte 12。
        assert_eq!(t.byte_at_point(30.0, 25.0), 12);
    }

    #[test]
    fn byte_at_point_clamps_y_above_and_below() {
        let t = table();
        // 上にはみ出す y は行0へ。
        assert_eq!(t.byte_at_point(0.0, -100.0), 0);
        // 下にはみ出す y は最終行へ。
        assert_eq!(t.byte_at_point(0.0, 999.0), 9);
    }

    /// 実 adapter が解決済み Parley レイアウトを正しく包むことの統合確認。フォント
    /// メトリクスに依らない不変量（行数・行範囲・x 単調性・端点ヒット）だけを見る。
    #[test]
    fn parley_adapter_wraps_real_layout() {
        use parley::{FontContext, LayoutContext};
        let mut font_cx = FontContext::new();
        let mut layout_cx: LayoutContext<TextBrush> = LayoutContext::new();
        let mut layout = layout_cx
            .ranged_builder(&mut font_cx, "hello", 1.0, true)
            .build("hello");
        // 行レイアウトは line-break して初めて生える（実 `content_layout` と同形）。
        layout.break_all_lines(None);

        let geo = ParleyCaretGeometry::new(&layout);
        assert_eq!(geo.line_count(), 1);
        assert_eq!(geo.line_bounds(0), (0, 5));
        // キャレット x は文頭から文末へ単調増加。
        assert!(geo.x_of(0) <= geo.x_of(5));
        assert!(geo.x_of(0) < geo.x_of(5));
        // 文頭直前の点は byte 0、原点は line 0。
        assert_eq!(geo.line_of(0), 0);
        assert_eq!(geo.byte_at_point(-10.0, 0.0), 0);
    }
}
