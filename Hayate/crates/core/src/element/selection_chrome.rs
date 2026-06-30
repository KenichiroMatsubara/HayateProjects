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

/// レイアウト済みのフローティング選択ツールバー。選択上に配置された、順序付き
/// ボタン列とバー全体の矩形。[`layout`] が生成し、ヒットテスト（入力）とシーン
/// 出力（描画）の双方が利用する。
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionToolbar {
    pub style: SelectionChromeStyle,
    pub bounds: ToolbarRect,
    pub buttons: Vec<ToolbarButton>,
}

impl SelectionToolbar {
    /// ツールバーのアクションを表示順で返す。
    pub fn actions(&self) -> Vec<ToolbarAction> {
        self.buttons.iter().map(|b| b.action).collect()
    }

    /// `(x, y)` を含むボタンのアクション。どのボタンにも当たらなければ `None`
    /// （ランタイムは押下を通常どおり扱う）。
    pub fn action_at(&self, x: f32, y: f32) -> Option<ToolbarAction> {
        self.buttons
            .iter()
            .find(|b| b.bounds.contains(x, y))
            .map(|b| b.action)
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
        let candidates = [(d2(&self.start), self.start.end), (d2(&self.end), self.end.end)];
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

/// Material ツールバーの寸法。core が描く単一 chrome で値はテーマ切替可能
/// （ADR-0097）。Material が初期テーマ。
pub(crate) const TOOLBAR_HEIGHT: f32 = 40.0;
pub(crate) const TOOLBAR_LABEL_FONT_SIZE: f32 = 14.0;
pub(crate) const TOOLBAR_CORNER_RADIUS: f32 = 4.0;

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
/// レイアウトと描画の間で自己整合する。
const LABEL_CHAR_ADVANCE: f32 = 8.0;
/// ボタンラベル左右のパディング。
const BUTTON_PAD_X: f32 = 12.0;
/// ツールバーと、その上に浮かぶ選択との間の縦ギャップ。
const TOOLBAR_GAP: f32 = 8.0;

fn button_width(action: ToolbarAction) -> f32 {
    action.label().chars().count() as f32 * LABEL_CHAR_ADVANCE + 2.0 * BUTTON_PAD_X
}

/// 選択のバウンディングボックス `sel`（canvas 座標）の上にツールバーを配置する。
/// 水平中央寄せで選択の真上に浮かべ、上端に余地がなければ下へ反転する。バーは
/// `viewport` 内に水平方向でクランプする。`actions` が空なら `None`。
pub(crate) fn layout(
    style: SelectionChromeStyle,
    actions: &[ToolbarAction],
    sel: ToolbarRect,
    viewport: (f32, f32),
) -> Option<SelectionToolbar> {
    if actions.is_empty() {
        return None;
    }
    let total_width: f32 = actions.iter().map(|&a| button_width(a)).sum();

    // 選択上に中央寄せし、viewport 内に水平クランプ。
    let center_x = sel.x + sel.width / 2.0;
    let max_x = (viewport.0 - total_width).max(0.0);
    let x = (center_x - total_width / 2.0).clamp(0.0, max_x);

    // 選択の上に浮かべるのを優先し、上端をはみ出すなら下へ反転。
    let above_y = sel.y - TOOLBAR_GAP - TOOLBAR_HEIGHT;
    let y = if above_y >= 0.0 {
        above_y
    } else {
        sel.y + sel.height + TOOLBAR_GAP
    };

    let mut buttons = Vec::with_capacity(actions.len());
    let mut bx = x;
    for &action in actions {
        let w = button_width(action);
        buttons.push(ToolbarButton {
            action,
            bounds: ToolbarRect {
                x: bx,
                y,
                width: w,
                height: TOOLBAR_HEIGHT,
            },
        });
        bx += w;
    }

    Some(SelectionToolbar {
        style,
        bounds: ToolbarRect {
            x,
            y,
            width: total_width,
            height: TOOLBAR_HEIGHT,
        },
        buttons,
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

    #[test]
    fn buttons_are_laid_out_left_to_right_in_action_order() {
        let actions = [ToolbarAction::Copy, ToolbarAction::SelectAll];
        let tb = layout(SelectionChromeStyle::Material, &actions, sel(100.0, 80.0, 60.0, 20.0), (400.0, 200.0))
            .expect("non-empty actions produce a toolbar");
        assert_eq!(tb.actions(), actions.to_vec());
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
        )
        .unwrap();
        assert_eq!(tb.bounds.y, 80.0 - TOOLBAR_GAP - TOOLBAR_HEIGHT);
    }

    #[test]
    fn toolbar_flips_below_when_there_is_no_room_above() {
        // 上端に張り付いた選択: 上だと負になるので下へ反転。
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Copy],
            sel(100.0, 2.0, 60.0, 20.0),
            (400.0, 200.0),
        )
        .unwrap();
        assert_eq!(tb.bounds.y, 2.0 + 20.0 + TOOLBAR_GAP);
    }

    #[test]
    fn toolbar_is_clamped_within_the_viewport_horizontally() {
        // 右端付近の選択: バーは viewport をはみ出してはならない。
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Cut, ToolbarAction::Copy, ToolbarAction::Paste, ToolbarAction::SelectAll],
            sel(390.0, 80.0, 8.0, 20.0),
            (400.0, 200.0),
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
        assert!(layout(SelectionChromeStyle::Material, &[], sel(0.0, 0.0, 0.0, 0.0), (400.0, 200.0)).is_none());
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

