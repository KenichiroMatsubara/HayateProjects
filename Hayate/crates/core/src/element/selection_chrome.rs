//! フローティング選択ツールバー — core が描画する選択 chrome（ADR-0097）。
//!
//! 選択 chrome（ハイライト・ハンドル・フローティングツールバー）は core が
//! SceneGraph へ一度だけ描画し、テーマ切替できるのは *style* のみ。OS ネイティブ
//! のツールバーウィジェットを Platform Adapter ごとに再実装しない。本モジュールは
//! スタイル非依存のツールバー **モデル**（どのアクションを出すか、ボタン配置、
//! タップがどのボタンに当たるか）と [`SelectionChromeStyle`] スイッチを持つ。

/// 選択ハイライト・ハンドル・フローティングツールバーの chrome テーマ。
/// Cupertino（iOS Platform Adapter 用）の追加が書き直しでなく加算で済むよう
/// 切替式にしてある（ADR-0097）。Material を最初に実装する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectionChromeStyle {
    /// Material Design 風 chrome（初期・デフォルトテーマ）。
    #[default]
    Material,
    /// Cupertino（iOS）風 chrome — iOS Platform Adapter とともに追加。
    Cupertino,
}

/// フローティング選択ツールバーのボタン。集合は選択内容で決まる。読み取り専用の
/// SelectionArea は読み取りアクション（Copy / Select All）のみ、編集可能なテキスト
/// 入力は変更アクション（Cut / Paste）も加える。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolbarAction {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl ToolbarAction {
    /// ツールバーに描画するボタンラベル。
    pub fn label(self) -> &'static str {
        match self {
            ToolbarAction::Cut => "Cut",
            ToolbarAction::Copy => "Copy",
            ToolbarAction::Paste => "Paste",
            ToolbarAction::SelectAll => "Select All",
        }
    }
}

/// canvas 座標系の軸並行矩形。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolbarRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ToolbarRect {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// フローティングツールバー上のタップ可能なボタン1個（canvas 座標の矩形付き）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolbarButton {
    pub action: ToolbarAction,
    pub bounds: ToolbarRect,
}

/// ⋮ オーバーフローメニュー（Android お手本）。可視バーに全ボタンが収まらないとき、
/// 末尾アクションを ⋮ トグルへ畳む。トグルを押すと、畳んだアクションを縦に並べた
/// 副メニューパネルが開く。[`SelectionToolbar`] のヒットテストは可視バーと、開いた
/// ときの副メニューの両方に当たる。
#[derive(Clone, Debug, PartialEq)]
pub struct OverflowMenu {
    /// バー末尾の ⋮ トグルボタン（canvas 座標）。押下で副メニューを開閉する。
    pub toggle: ToolbarRect,
    /// 副メニューが現在開いているか。
    pub open: bool,
    /// 開いたときの副メニューパネル全体の矩形（背景・elevation の描画範囲）。
    pub panel: ToolbarRect,
    /// 畳まれたアクション。パネル内に縦積みされ、`open` のときヒットテスト対象。
    pub items: Vec<ToolbarButton>,
}

/// フローティングツールバーへのヒットテスト結果。実アクション（Cut/Copy/…）か、
/// 副メニューを開閉する ⋮ オーバーフロートグルか。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarHit {
    /// 実アクションボタン（可視バー、または開いた副メニュー項目）。
    Action(ToolbarAction),
    /// ⋮ オーバーフロートグル — 押下で副メニューを開閉する。
    Overflow,
}

/// レイアウト済みのフローティング選択ツールバー。選択上に配置された、順序付き
/// ボタン列とバー全体の矩形。[`layout`] が生成し、ヒットテスト（入力）とシーン
/// 出力（描画）の双方が利用する。ボタン総幅が viewport を超えると末尾アクションを
/// `overflow` の ⋮ メニューへ畳む。
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionToolbar {
    pub style: SelectionChromeStyle,
    pub bounds: ToolbarRect,
    pub buttons: Vec<ToolbarButton>,
    /// 畳みが起きたときの ⋮ オーバーフローメニュー。全ボタンが収まれば `None`。
    pub overflow: Option<OverflowMenu>,
}

impl SelectionToolbar {
    /// ツールバーのアクションを表示順で返す。畳まれたアクションも（可視ボタンの後ろに）
    /// 続けて含む。
    pub fn actions(&self) -> Vec<ToolbarAction> {
        self.buttons
            .iter()
            .map(|b| b.action)
            .chain(
                self.overflow
                    .iter()
                    .flat_map(|of| of.items.iter().map(|b| b.action)),
            )
            .collect()
    }

    /// `(x, y)` を含むアクションボタンのアクション。可視バーに当たればそれ、開いた
    /// 副メニュー項目に当たればそれを返す。⋮ トグルやどのボタンにも当たらなければ
    /// `None`（ランタイムは押下を通常どおり扱う）。
    pub fn action_at(&self, x: f32, y: f32) -> Option<ToolbarAction> {
        if let Some(b) = self.buttons.iter().find(|b| b.bounds.contains(x, y)) {
            return Some(b.action);
        }
        if let Some(of) = &self.overflow {
            if of.open {
                if let Some(b) = of.items.iter().find(|b| b.bounds.contains(x, y)) {
                    return Some(b.action);
                }
            }
        }
        None
    }

    /// `(x, y)` のヒットテスト。アクションボタン（[`action_at`](Self::action_at) と同じ
    /// 範囲）か ⋮ オーバーフロートグルか。どれにも当たらなければ `None`。
    pub fn hit_test(&self, x: f32, y: f32) -> Option<ToolbarHit> {
        if let Some(action) = self.action_at(x, y) {
            return Some(ToolbarHit::Action(action));
        }
        if let Some(of) = &self.overflow {
            if of.toggle.contains(x, y) {
                return Some(ToolbarHit::Overflow);
            }
        }
        None
    }
}

/// ドラッグハンドルが操作する範囲の端。`Start` はドキュメント上で前方の端点、
/// `End` は後方の端点を調整する（ADR-0097）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionHandleEnd {
    Start,
    End,
}

/// Material のしずく型ドラッグハンドル1個。選択のキャレット端から外側へ下がる
/// しずく型のつまみで、ドラッグでその端点を調整する（ADR-0097）。形状はスタイル
/// 非依存で、テーマは色付けのみ行う。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionHandle {
    pub end: SelectionHandleEnd,
    /// しずくの本体（bulb）中心（canvas 座標）— 円形のグラブ対象。キャレット端から
    /// 外側へ `radius` ずらしてあり、tip（角の隅）が選択の端に触れる。
    pub knob_x: f32,
    pub knob_y: f32,
    /// 見えるつまみの半径。
    pub radius: f32,
}

impl SelectionHandle {
    /// しずく本体の描画ボックス（canvas 座標の `(x, y, width, height)`）。bulb 中心
    /// `(knob_x, knob_y)` を囲む `2*radius` 四方。
    pub fn draw_box(self) -> (f32, f32, f32, f32) {
        let d = self.radius * 2.0;
        (self.knob_x - self.radius, self.knob_y - self.radius, d, d)
    }

    /// しずく型を作るボックスの角丸（TL, TR, BR, BL）。Chrome Android お手本では、
    /// 選択側を向く上の隅だけを角（半径 0 の tip）にし、残り 3 隅を半径 `radius` で
    /// 丸める。`Start`（左端）は tip が右上、`End`（右端）は tip が左上の左右ミラー。
    pub fn corner_radii(self) -> [f32; 4] {
        let r = self.radius;
        match self.end {
            // tip = 右上 → 本体は左下へ。
            SelectionHandleEnd::Start => [r, 0.0, r, r],
            // tip = 左上 → 本体は右下へ。
            SelectionHandleEnd::End => [0.0, r, r, r],
        }
    }
}

/// アクティブな選択を挟む Material ドラッグハンドルの対（ADR-0097）。範囲の各端に
/// 1個ずつ。[`layout_handles`] が生成し、ヒットテスト（ハンドルドラッグ）とシーン
/// 出力（描画）の双方が利用する。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionHandles {
    pub style: SelectionChromeStyle,
    pub start: SelectionHandle,
    pub end: SelectionHandle,
}

impl SelectionHandles {
    /// `(x, y)` がつかむハンドルの端。どちらにも届かなければ `None`。両方のつまみが
    /// 届く範囲（極めて短い選択）では近い方が勝つので、どちらの端も狙える。
    pub fn handle_at(&self, x: f32, y: f32) -> Option<SelectionHandleEnd> {
        let d2 = |h: &SelectionHandle| {
            let dx = x - h.knob_x;
            let dy = y - h.knob_y;
            dx * dx + dy * dy
        };
        let reach = HANDLE_HIT_RADIUS * HANDLE_HIT_RADIUS;
        let candidates = [
            (d2(&self.start), self.start.end),
            (d2(&self.end), self.end.end),
        ];
        candidates
            .into_iter()
            .filter(|&(dist, _)| dist <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, end)| end)
    }
}

/// Material 選択ハンドルのつまみの見える半径。
pub(crate) const HANDLE_RADIUS: f32 = 8.0;
/// ハンドルをつかむヒット半径 — 指で当てられるよう、つまみより大きい
/// （Material ハンドルのタッチ対象は見える点よりはるかに大きい）。
pub(crate) const HANDLE_HIT_RADIUS: f32 = 22.0;

/// 選択の両端のキャレット端（canvas 座標の `(x, baseline_bottom_y)`）から Material
/// ドラッグハンドル2個を配置する。各しずくの tip（角の隅）はキャレット端に置き、
/// 本体は選択の*外側*へ斜め下にぶら下げる（Chrome Android お手本）。`Start` は左下へ、
/// `End` は右下へ。これで本体が選択の先頭/末尾グリフに被らず、選択を外から囲む。
pub(crate) fn layout_handles(
    style: SelectionChromeStyle,
    start_caret: (f32, f32),
    end_caret: (f32, f32),
) -> SelectionHandles {
    let handle = |end: SelectionHandleEnd, (cx, cy): (f32, f32)| {
        // bulb 中心をキャレット端から外側へ radius ずらすと、tip（内側上の隅）が
        // ちょうどキャレット端 `(cx, cy)` に来る。
        let knob_x = match end {
            SelectionHandleEnd::Start => cx - HANDLE_RADIUS,
            SelectionHandleEnd::End => cx + HANDLE_RADIUS,
        };
        SelectionHandle {
            end,
            knob_x,
            knob_y: cy + HANDLE_RADIUS,
            radius: HANDLE_RADIUS,
        }
    };
    SelectionHandles {
        style,
        start: handle(SelectionHandleEnd::Start, start_caret),
        end: handle(SelectionHandleEnd::End, end_caret),
    }
}

impl SelectionChromeStyle {
    /// 選択ドラッグハンドルの塗り色（RGBA, 0..1）。
    pub(crate) fn handle_color(self) -> [f32; 4] {
        match self {
            // Material: ハイライトに合わせたプライマリ選択ブルー。
            SelectionChromeStyle::Material => [0.20, 0.45, 0.95, 1.0],
            SelectionChromeStyle::Cupertino => [0.0, 0.48, 1.0, 1.0],
        }
    }
}

/// Material ツールバーの視覚値の正本デフォルト（ADR-0097）。これらは
/// [`ChromeTuning`](crate::element::chrome_tuning::ChromeTuning) の名前付きフィールドへ
/// 昇格しており、`tuning.json` で再ビルドなしに上書きできる。ここが唯一の出所で、
/// `ChromeTuning::default` が読む。Material の色（divider/shadow）は実機 Chrome Android
/// 較正前のプレースホルダ。
pub(crate) const TOOLBAR_HEIGHT: f32 = 40.0;
pub(crate) const TOOLBAR_LABEL_FONT_SIZE: f32 = 14.0;
pub(crate) const TOOLBAR_CORNER_RADIUS: f32 = 4.0;
/// ボタンラベル左右のパディング。
pub(crate) const TOOLBAR_BUTTON_PAD_X: f32 = 12.0;
/// ツールバーと、その上（または下）に浮かぶ選択との間の縦ギャップ。
pub(crate) const TOOLBAR_GAP: f32 = 8.0;
/// ボタン間の Material 区切り線の幅と色（非プリマルチプライ RGBA, 0..1）。色は新規
/// affordance なので tunable（既存のパネル/ラベル色はテーマ所有のまま）。
pub(crate) const TOOLBAR_DIVIDER_WIDTH: f32 = 1.0;
pub(crate) const TOOLBAR_DIVIDER_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.12];
/// パネルの Material elevation（drop shadow）パラメータ。既存の box-shadow lowering で
/// 描く。色は新規 affordance なので tunable。
pub(crate) const TOOLBAR_ELEVATION_OFFSET_Y: f32 = 2.0;
pub(crate) const TOOLBAR_ELEVATION_BLUR: f32 = 8.0;
pub(crate) const TOOLBAR_ELEVATION_SPREAD: f32 = 0.0;
pub(crate) const TOOLBAR_SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.35];

/// ⋮ オーバーフロートグルのグリフ。1文字幅のボタンとして扱う。
pub(crate) const OVERFLOW_GLYPH: &str = "⋮";

/// 下へ反転したツールバーが避けるべき、選択下のドラッグハンドル到達量。しずくは
/// キャレット端から本体半径ぶん下がり、さらに本体が半径ぶん伸びるので `2*radius`
/// 下まで届く（ADR-0104）。反転時はこのぶんだけ余計に下げてハンドルに重ねない。
pub(crate) const HANDLE_REACH_BELOW: f32 = HANDLE_RADIUS * 2.0;

/// [`layout`] が読むツールバーの視覚メトリクス。[`ChromeTuning`] 由来で、`tuning.json`
/// による再ビルド不要の上書きを反映する（旧来の const 直読みを置き換え）。
///
/// [`ChromeTuning`]: crate::element::chrome_tuning::ChromeTuning
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ToolbarMetrics {
    pub height: f32,
    pub button_pad_x: f32,
    pub gap: f32,
}

impl SelectionChromeStyle {
    /// ツールバーパネルの背景色（非プリマルチプライ RGBA, 0..1）。
    pub(crate) fn toolbar_background(self) -> [f32; 4] {
        match self {
            // Material: ほぼ不透明な暗いサーフェス。
            SelectionChromeStyle::Material => [0.20, 0.20, 0.22, 0.98],
            SelectionChromeStyle::Cupertino => [0.18, 0.18, 0.18, 0.96],
        }
    }

    /// ツールバーラベルのテキスト色（RGBA, 0..1）。
    pub(crate) fn toolbar_label(self) -> [f32; 4] {
        match self {
            SelectionChromeStyle::Material => [0.98, 0.98, 0.98, 1.0],
            SelectionChromeStyle::Cupertino => [1.0, 1.0, 1.0, 1.0],
        }
    }
}
/// ラベル1文字あたりの概算水平送り。core が自前でラベルを描くので、この見積もりは
/// レイアウトと描画の間で自己整合する。レイアウトジオメトリ（tree 非経由で読む）なので
/// tunable にせず const のまま（`ChromeTuning` ドキュメント参照）。
const LABEL_CHAR_ADVANCE: f32 = 8.0;

fn label_width(label: &str, pad_x: f32) -> f32 {
    label.chars().count() as f32 * LABEL_CHAR_ADVANCE + 2.0 * pad_x
}

fn button_width(action: ToolbarAction, pad_x: f32) -> f32 {
    label_width(action.label(), pad_x)
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> ToolbarRect {
    ToolbarRect {
        x,
        y,
        width,
        height,
    }
}

/// `width` のバーを選択上に中央寄せし、`viewport` 幅内へ水平クランプした左 x。
fn centered_x(sel: ToolbarRect, width: f32, viewport_w: f32) -> f32 {
    let center_x = sel.x + sel.width / 2.0;
    let max_x = (viewport_w - width).max(0.0);
    (center_x - width / 2.0).clamp(0.0, max_x)
}

/// バーの縦位置。選択の上に浮かべるのを優先し、上端に余地が無ければ下へ反転する。
/// 下反転時は選択下にぶら下がるドラッグハンドル（ADR-0104）に重ならないよう
/// [`HANDLE_REACH_BELOW`] ぶん余計に下げてクリアする。
fn placement_y(sel: ToolbarRect, height: f32, gap: f32) -> f32 {
    let above_y = sel.y - gap - height;
    if above_y >= 0.0 {
        above_y
    } else {
        sel.y + sel.height + HANDLE_REACH_BELOW + gap
    }
}

/// `actions`/`widths` を `x` から左→右に1行へ並べる。
fn lay_out_row(
    actions: &[ToolbarAction],
    widths: &[f32],
    x: f32,
    y: f32,
    height: f32,
) -> Vec<ToolbarButton> {
    let mut buttons = Vec::with_capacity(actions.len());
    let mut bx = x;
    for (&action, &w) in actions.iter().zip(widths) {
        buttons.push(ToolbarButton {
            action,
            bounds: rect(bx, y, w, height),
        });
        bx += w;
    }
    buttons
}

/// 選択のバウンディングボックス `sel`（canvas 座標）の上にツールバーを配置する。
/// 水平中央寄せで選択の真上に浮かべ、上端に余地がなければ下へ反転（ハンドル回避込み）。
/// バーは `viewport` 内に水平クランプする。ボタン総幅が viewport を超えるときは
/// 末尾アクションを ⋮ オーバーフローメニューへ畳み、`overflow_open` のとき副メニューを
/// 展開する。視覚値は `metrics`（[`ChromeTuning`] 由来）から取り、インラインの
/// マジックナンバーを持たない。`actions` が空なら `None`。
///
/// [`ChromeTuning`]: crate::element::chrome_tuning::ChromeTuning
pub(crate) fn layout(
    style: SelectionChromeStyle,
    actions: &[ToolbarAction],
    sel: ToolbarRect,
    viewport: (f32, f32),
    metrics: ToolbarMetrics,
    overflow_open: bool,
) -> Option<SelectionToolbar> {
    if actions.is_empty() {
        return None;
    }
    let pad = metrics.button_pad_x;
    let h = metrics.height;
    let widths: Vec<f32> = actions.iter().map(|&a| button_width(a, pad)).collect();
    let total_width: f32 = widths.iter().sum();

    // 全ボタンが収まる: 従来どおり1行に並べる（オーバーフロー無し）。
    if total_width <= viewport.0 {
        let x = centered_x(sel, total_width, viewport.0);
        let y = placement_y(sel, h, metrics.gap);
        let buttons = lay_out_row(actions, &widths, x, y, h);
        return Some(SelectionToolbar {
            style,
            bounds: rect(x, y, total_width, h),
            buttons,
            overflow: None,
        });
    }

    // オーバーフロー: 先頭アクションを ⋮ トグルぶんの余地を残しつつ貪欲に詰め、
    // 残りを副メニューへ畳む（Android お手本）。
    let toggle_w = label_width(OVERFLOW_GLYPH, pad);
    let mut visible_count = 0;
    let mut used = 0.0_f32;
    for (i, &w) in widths.iter().enumerate() {
        if used + w + toggle_w <= viewport.0 {
            used += w;
            visible_count = i + 1;
        } else {
            break;
        }
    }
    let bar_width = used + toggle_w;
    let x = centered_x(sel, bar_width, viewport.0);
    let y = placement_y(sel, h, metrics.gap);

    let buttons = lay_out_row(&actions[..visible_count], &widths[..visible_count], x, y, h);

    // ⋮ トグルは可視ボタン列の末尾。
    let toggle_x = x + used;
    let toggle = rect(toggle_x, y, toggle_w, h);

    // 畳んだアクションを縦積みの副メニューにする。パネル幅は最も広い項目（最低でも
    // トグル幅）に合わせ、トグル右端に右揃えして viewport 内へクランプ、バーの直下に置く。
    let folded = &actions[visible_count..];
    let menu_w = folded
        .iter()
        .map(|&a| button_width(a, pad))
        .fold(toggle_w, f32::max);
    let menu_x = (toggle_x + toggle_w - menu_w).clamp(0.0, (viewport.0 - menu_w).max(0.0));
    let menu_top = y + h;
    let items: Vec<ToolbarButton> = folded
        .iter()
        .enumerate()
        .map(|(i, &action)| ToolbarButton {
            action,
            bounds: rect(menu_x, menu_top + i as f32 * h, menu_w, h),
        })
        .collect();
    let panel = rect(menu_x, menu_top, menu_w, h * folded.len() as f32);

    Some(SelectionToolbar {
        style,
        bounds: rect(x, y, bar_width, h),
        buttons,
        overflow: Some(OverflowMenu {
            toggle,
            open: overflow_open,
            panel,
            items,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(x: f32, y: f32, w: f32, h: f32) -> ToolbarRect {
        ToolbarRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// 正本 const を反映するレイアウトメトリクス（`ChromeTuning::default` と同じ値）。
    fn metrics() -> ToolbarMetrics {
        ToolbarMetrics {
            height: TOOLBAR_HEIGHT,
            button_pad_x: TOOLBAR_BUTTON_PAD_X,
            gap: TOOLBAR_GAP,
        }
    }

    #[test]
    fn buttons_are_laid_out_left_to_right_in_action_order() {
        let actions = [ToolbarAction::Copy, ToolbarAction::SelectAll];
        let tb = layout(
            SelectionChromeStyle::Material,
            &actions,
            sel(100.0, 80.0, 60.0, 20.0),
            (400.0, 200.0),
            metrics(),
            false,
        )
        .expect("non-empty actions produce a toolbar");
        assert_eq!(tb.actions(), actions.to_vec());
        assert!(
            tb.overflow.is_none(),
            "everything fits, so no overflow menu"
        );
        // 各ボタンは前のボタンのすぐ右に並び、重ならない。
        let a = tb.buttons[0].bounds;
        let b = tb.buttons[1].bounds;
        assert_eq!(b.x, a.x + a.width);
    }

    #[test]
    fn toolbar_floats_above_the_selection_with_a_gap() {
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Copy],
            sel(100.0, 80.0, 60.0, 20.0),
            (400.0, 200.0),
            metrics(),
            false,
        )
        .unwrap();
        assert_eq!(tb.bounds.y, 80.0 - TOOLBAR_GAP - TOOLBAR_HEIGHT);
    }

    #[test]
    fn toolbar_flips_below_clearing_the_drag_handles() {
        // 上端に張り付いた選択: 上だと負になるので下へ反転。反転時は選択下に
        // ぶら下がるドラッグハンドルに重ならないよう handle reach ぶん余計に下げる。
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Copy],
            sel(100.0, 2.0, 60.0, 20.0),
            (400.0, 200.0),
            metrics(),
            false,
        )
        .unwrap();
        assert_eq!(tb.bounds.y, 2.0 + 20.0 + HANDLE_REACH_BELOW + TOOLBAR_GAP);
        // ハンドル（選択下端から HANDLE_REACH_BELOW まで届く）の下に確実に出る。
        let handle_bottom = 2.0 + 20.0 + HANDLE_REACH_BELOW;
        assert!(
            tb.bounds.y >= handle_bottom,
            "flipped toolbar clears the drag handle below the selection",
        );
    }

    #[test]
    fn toolbar_is_clamped_within_the_viewport_horizontally() {
        // 右端付近の選択: バーは viewport をはみ出してはならない。
        let tb = layout(
            SelectionChromeStyle::Material,
            &[
                ToolbarAction::Cut,
                ToolbarAction::Copy,
                ToolbarAction::Paste,
                ToolbarAction::SelectAll,
            ],
            sel(390.0, 80.0, 8.0, 20.0),
            (400.0, 200.0),
            metrics(),
            false,
        )
        .unwrap();
        assert!(tb.bounds.x >= 0.0);
        assert!(tb.bounds.x + tb.bounds.width <= 400.0 + 0.01);
    }

    #[test]
    fn action_at_hits_the_button_under_the_point() {
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Copy, ToolbarAction::SelectAll],
            sel(100.0, 80.0, 60.0, 20.0),
            (400.0, 200.0),
            metrics(),
            false,
        )
        .unwrap();
        let copy = tb.buttons[0].bounds;
        assert_eq!(
            tb.action_at(copy.x + 1.0, copy.y + 1.0),
            Some(ToolbarAction::Copy),
        );
        // バーの上の点は何にも当たらない。
        assert_eq!(tb.action_at(copy.x + 1.0, copy.y - 5.0), None);
    }

    #[test]
    fn empty_actions_produce_no_toolbar() {
        assert!(layout(
            SelectionChromeStyle::Material,
            &[],
            sel(0.0, 0.0, 0.0, 0.0),
            (400.0, 200.0),
            metrics(),
            false,
        )
        .is_none());
    }

    /// オーバーフローを強制する、全アクションが収まらない狭い viewport を組む。
    fn overflowing_toolbar(open: bool) -> SelectionToolbar {
        let actions = [
            ToolbarAction::Cut,
            ToolbarAction::Copy,
            ToolbarAction::Paste,
            ToolbarAction::SelectAll,
        ];
        let m = metrics();
        let total: f32 = actions
            .iter()
            .map(|&a| button_width(a, m.button_pad_x))
            .sum();
        // 全ボタン幅の半分しか無い viewport にして必ず畳ませる。
        let viewport_w = total / 2.0;
        layout(
            SelectionChromeStyle::Material,
            &actions,
            sel(0.0, 80.0, 10.0, 20.0),
            (viewport_w, 200.0),
            m,
            open,
        )
        .expect("non-empty actions produce a toolbar")
    }

    #[test]
    fn narrow_viewport_folds_trailing_actions_into_an_overflow_menu() {
        let actions = [
            ToolbarAction::Cut,
            ToolbarAction::Copy,
            ToolbarAction::Paste,
            ToolbarAction::SelectAll,
        ];
        let viewport_w: f32 = actions
            .iter()
            .map(|&a| button_width(a, metrics().button_pad_x))
            .sum::<f32>()
            / 2.0;
        let tb = overflowing_toolbar(false);
        let overflow = tb.overflow.as_ref().expect("a narrow bar overflows");
        // バーは viewport をはみ出さない（畳んだので水平クランプではなく ⋮ に収まる）。
        assert!(tb.bounds.x >= 0.0);
        assert!(tb.bounds.x + tb.bounds.width <= viewport_w + 0.01);
        assert!(!overflow.items.is_empty(), "some actions are folded away");
        // 可視ボタンと畳まれた項目を合わせると、元のアクション集合が順序どおり揃う。
        assert_eq!(
            tb.actions(),
            vec![
                ToolbarAction::Cut,
                ToolbarAction::Copy,
                ToolbarAction::Paste,
                ToolbarAction::SelectAll,
            ],
        );
        // 末尾のアクションが畳まれる（先頭から詰めるので Select All は副メニュー側）。
        assert_eq!(
            overflow.items.last().unwrap().action,
            ToolbarAction::SelectAll
        );
    }

    #[test]
    fn overflow_submenu_is_hit_tested_only_when_open() {
        let closed = overflowing_toolbar(false);
        let item = closed.overflow.as_ref().unwrap().items[0].bounds;
        let (ix, iy) = (item.x + 1.0, item.y + 1.0);
        // 閉じているとき: 副メニュー位置の点はどのアクションにも当たらない。
        assert_eq!(closed.action_at(ix, iy), None);

        let open = overflowing_toolbar(true);
        let opened_item = open.overflow.as_ref().unwrap().items[0];
        let b = opened_item.bounds;
        // 開いているとき: 同じ点が畳まれたアクションに当たる。
        assert_eq!(
            open.action_at(b.x + 1.0, b.y + 1.0),
            Some(opened_item.action),
        );
    }

    #[test]
    fn hit_test_distinguishes_the_overflow_toggle_from_actions() {
        let tb = overflowing_toolbar(false);
        let toggle = tb.overflow.as_ref().unwrap().toggle;
        // ⋮ トグルはアクションではなくオーバーフローヒットとして返る。
        assert_eq!(
            tb.hit_test(toggle.x + 1.0, toggle.y + 1.0),
            Some(ToolbarHit::Overflow),
        );
        assert_eq!(tb.action_at(toggle.x + 1.0, toggle.y + 1.0), None);
        // 可視バーのボタンは通常どおりアクションとして当たる。
        let first = tb.buttons[0].bounds;
        assert_eq!(
            tb.hit_test(first.x + 1.0, first.y + 1.0),
            Some(ToolbarHit::Action(tb.buttons[0].action)),
        );
    }

    #[test]
    fn handles_hang_outward_below_both_selection_ends() {
        // 一行範囲の両端のキャレット端は同じベースラインを共有し、しずくの本体は
        // その下に、各端の*外側*（start は左、end は右）へ半径ぶんずれて下がる。
        // tip（角の隅）がちょうどキャレット端に来て、選択を外から囲む。
        let h = layout_handles(SelectionChromeStyle::Material, (10.0, 20.0), (80.0, 20.0));
        assert_eq!(h.start.end, SelectionHandleEnd::Start);
        assert_eq!(h.end.end, SelectionHandleEnd::End);
        // 本体中心はキャレット端から外側へ radius オフセット。
        assert_eq!(h.start.knob_x, 10.0 - HANDLE_RADIUS);
        assert_eq!(h.end.knob_x, 80.0 + HANDLE_RADIUS);
        assert!(h.start.knob_y > 20.0, "knob hangs below the text edge");
        assert_eq!(h.start.knob_y, h.end.knob_y);
        // 左右ミラーのしずく: start の tip は右上、end の tip は左上。
        let r = HANDLE_RADIUS;
        assert_eq!(h.start.corner_radii(), [r, 0.0, r, r]);
        assert_eq!(h.end.corner_radii(), [0.0, r, r, r]);
        // tip は選択の各端に触れる: start 本体の右端、end 本体の左端がキャレット x。
        let (sx, _, sw, _) = h.start.draw_box();
        let (ex, _, _, _) = h.end.draw_box();
        assert_eq!(sx + sw, 10.0, "start tip sits at the left selection edge");
        assert_eq!(ex, 80.0, "end tip sits at the right selection edge");
    }

    #[test]
    fn handle_at_picks_the_end_under_the_point() {
        let h = layout_handles(SelectionChromeStyle::Material, (10.0, 20.0), (80.0, 20.0));
        assert_eq!(
            h.handle_at(h.start.knob_x, h.start.knob_y),
            Some(SelectionHandleEnd::Start),
        );
        assert_eq!(
            h.handle_at(h.end.knob_x, h.end.knob_y),
            Some(SelectionHandleEnd::End),
        );
        // 両つまみから遠い点はどちらもつかまない。
        assert_eq!(h.handle_at(45.0, 400.0), None);
    }
}
