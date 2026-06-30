//! Material のフローティング選択ツールバー（core が描く選択クローム、ADR-0097）。
//! ツールバーのアクション・ジオメトリ・ヒットテスト・クリップボード配線を公開
//! `ElementTree` インターフェース越しに検証する。

use std::cell::RefCell;
use std::rc::Rc;

use hayate_core::{
    ChromeTuning, Clipboard, Dimension, DrawOp, ElementId, ElementKind, ElementTree, PointerKind,
    RecordingPainter, StyleProp, ToolbarAction, render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// 書き込みを記録し、あらかじめ設定した読み取り値を返す `Clipboard` のダブル。
/// ツールバーがアダプタ境界越しに何を渡し/取ったかをテストで検証できる。
#[derive(Default, Clone)]
struct FakeClipboard {
    writes: Rc<RefCell<Vec<String>>>,
    read: Rc<RefCell<Option<String>>>,
}

impl Clipboard for FakeClipboard {
    fn write_text(&self, text: &str) {
        self.writes.borrow_mut().push(text.to_string());
    }
    fn read_text(&self) -> Option<String> {
        self.read.borrow().clone()
    }
}

/// `action` のツールバーボタンを中心で押す（アクションは押下で発火）。
/// ツールバーや該当ボタンが出ていなければパニックする。
fn tap(tree: &mut ElementTree, action: ToolbarAction) {
    let button = tree
        .selection_toolbar()
        .expect("a toolbar is showing")
        .buttons
        .into_iter()
        .find(|b| b.action == action)
        .expect("the requested action's button is present");
    let b = button.bounds;
    tree.on_pointer_down(b.x + b.width / 2.0, b.y + b.height / 2.0);
}

/// `<view [selectable]><text "Hello world"></view>` を1行で組み、
/// (tree, view, text) を返す。`text_selection.rs` のハーネスに倣う。
fn selectable_paragraph() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, text);
    tree.element_set_text(text, "Hello world");
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, view, text)
}

/// タッチでドラッグ選択して離す。Touch モダリティで非空の読み取り専用選択を
/// 残すため、クロームが表示される（ADR-0104）。
fn select_a_range(tree: &mut ElementTree) {
    tree.on_pointer_down_with_kind(2.0, 8.0, 0, PointerKind::Touch);
    tree.on_pointer_move(70.0, 8.0);
    tree.on_pointer_up(70.0, 8.0);
}

/// `content` を持つ、レイアウト済みでフォーカスされた text-input。(tree, input) を返す。
fn text_input_with(content: &str) -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let input = tree.element_create(20, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_append_text_content(input, content);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input)
}

#[test]
fn read_only_selection_offers_copy_and_select_all() {
    let (mut tree, _view, _text) = selectable_paragraph();
    select_a_range(&mut tree);

    let toolbar = tree
        .selection_toolbar()
        .expect("a toolbar appears over a non-empty selection");

    assert_eq!(
        toolbar.actions(),
        vec![ToolbarAction::Copy, ToolbarAction::SelectAll],
        "a read-only SelectionArea offers Copy then Select All",
    );
}

#[test]
fn toolbar_is_drawn_by_core_during_selection() {
    let (mut tree, _view, _text) = selectable_paragraph();
    select_a_range(&mut tree);
    tree.render(0.0);

    let toolbar = tree.selection_toolbar().expect("a toolbar");
    let ops = draw_ops(&tree);

    // ツールバーの背景パネルは bounds の位置に塗りつぶし矩形として描かれる。
    let panel = ops.iter().find(|op| {
        matches!(
            op,
            DrawOp::FillRect { x, y, width, height, corner_radius, .. }
                if (*x - toolbar.bounds.x).abs() < 0.5
                    && (*y - toolbar.bounds.y).abs() < 0.5
                    && (*width - toolbar.bounds.width).abs() < 0.5
                    && (*height - toolbar.bounds.height).abs() < 0.5
                    && *corner_radius > 0.0
        )
    });
    assert!(panel.is_some(), "the toolbar panel is drawn at its bounds");
}

#[test]
fn toolbar_disappears_from_the_scene_when_the_selection_clears() {
    let (mut tree, _view, _text) = selectable_paragraph();
    select_a_range(&mut tree);
    tree.render(0.0);
    let bounds = tree.selection_toolbar().expect("a toolbar").bounds;
    let panel_count = |ops: &[DrawOp]| {
        ops.iter()
            .filter(|op| {
                matches!(op, DrawOp::FillRect { x, y, .. }
                    if (*x - bounds.x).abs() < 0.5 && (*y - bounds.y).abs() < 0.5)
            })
            .count()
    };
    assert_eq!(panel_count(&draw_ops(&tree)), 1, "toolbar present while selecting");

    // 空白部分をクリックして選択を解除し、再描画する。
    tree.on_pointer_down(2.0, 150.0);
    tree.on_pointer_up(2.0, 150.0);
    tree.render(0.0);

    assert!(tree.selection_toolbar().is_none(), "selection cleared");
    assert_eq!(
        panel_count(&draw_ops(&tree)),
        0,
        "the overlay is removed from the scene once the selection clears",
    );
}

#[test]
fn chrome_style_switch_changes_the_toolbar_panel_and_is_additive() {
    use hayate_core::SelectionChromeStyle;

    let panel_color = |style: SelectionChromeStyle| -> [f32; 4] {
        let (mut tree, _v, _t) = selectable_paragraph();
        tree.set_selection_chrome_style(style);
        select_a_range(&mut tree);
        tree.render(0.0);
        let bounds = tree.selection_toolbar().expect("a toolbar").bounds;
        draw_ops(&tree)
            .into_iter()
            .find_map(|op| match op {
                DrawOp::FillRect { x, y, color, .. }
                    if (x - bounds.x).abs() < 0.5 && (y - bounds.y).abs() < 0.5 =>
                {
                    Some(color)
                }
                _ => None,
            })
            .expect("the toolbar panel rect")
    };

    // Material が既定。Cupertino への切り替えは加算的（enum がツールバーモデルを
    // 変えずに別テーマを選ぶ）で、見た目の異なるパネルになる。
    assert_eq!(SelectionChromeStyle::default(), SelectionChromeStyle::Material);
    assert_ne!(
        panel_color(SelectionChromeStyle::Material),
        panel_color(SelectionChromeStyle::Cupertino),
        "the chrome style enum drives a visibly different toolbar",
    );
}

#[test]
fn no_toolbar_without_a_selection() {
    let (tree, _view, _text) = selectable_paragraph();
    assert!(
        tree.selection_toolbar().is_none(),
        "no selection means no toolbar",
    );
}

#[test]
fn editable_text_input_selection_offers_cut_copy_paste_select_all() {
    let (mut tree, input) = text_input_with("hello world");

    // フィールド内をタッチでドラッグ選択する（クロームは Touch ゲート）。
    tree.on_pointer_down_with_kind(2.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_move(60.0, 20.0);
    tree.on_pointer_up(60.0, 20.0);
    assert!(tree.element_text_selection(input).is_some());

    let toolbar = tree
        .selection_toolbar()
        .expect("a toolbar appears over an editable selection");

    assert_eq!(
        toolbar.actions(),
        vec![
            ToolbarAction::Cut,
            ToolbarAction::Copy,
            ToolbarAction::Paste,
            ToolbarAction::SelectAll,
        ],
        "an editable text-input adds the mutating actions",
    );
}

#[test]
fn tapping_copy_writes_the_selection_through_the_clipboard() {
    let (mut tree, _view, _text) = selectable_paragraph();
    let clipboard = FakeClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));
    select_a_range(&mut tree);
    let expected = tree.selected_text().expect("a non-empty selection");

    tap(&mut tree, ToolbarAction::Copy);

    assert_eq!(clipboard.writes.borrow().as_slice(), &[expected]);
    // Copy は選択を残す。ツールバーはまだ表示されている。
    assert!(tree.selection_toolbar().is_some());
}

#[test]
fn tapping_select_all_extends_the_read_only_selection_to_the_whole_region() {
    let (mut tree, _view, text) = selectable_paragraph();
    select_a_range(&mut tree);

    tap(&mut tree, ToolbarAction::SelectAll);

    let sel = tree.selection().expect("a selection");
    let (start, end) = sel.range_within(text).expect("both ends in the text");
    assert_eq!((start, end), (0, "Hello world".len()));
}

#[test]
fn tapping_cut_copies_then_removes_the_editable_range() {
    let (mut tree, input) = text_input_with("hello world");
    let clipboard = FakeClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));

    // 先頭の非空範囲をタッチでドラッグ選択して Cut する（クロームは Touch ゲート）。
    tree.on_pointer_down_with_kind(2.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_move(60.0, 20.0);
    tree.on_pointer_up(60.0, 20.0);
    let content = tree.element_get_text_content(input);
    let (s, e) = tree.element_text_selection(input).expect("a non-empty range");
    let cut_text = content[s..e].to_string();
    let mut expected = content.clone();
    expected.replace_range(s..e, "");

    tap(&mut tree, ToolbarAction::Cut);

    assert_eq!(clipboard.writes.borrow().as_slice(), &[cut_text]);
    assert_eq!(
        tree.element_get_text_content(input),
        expected,
        "cutting removes exactly the selected range",
    );
}

/// 描画された FillRect の `(x, y, width, height, color, corner_radius)` を順序付きで返す。
fn fill_rects(tree: &ElementTree) -> Vec<(usize, f32, f32, f32, f32, [f32; 4], f32)> {
    draw_ops(tree)
        .into_iter()
        .enumerate()
        .filter_map(|(i, op)| match op {
            DrawOp::FillRect { x, y, width, height, color, corner_radius } => {
                Some((i, x, y, width, height, color, corner_radius))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_panel_is_drawn_with_a_material_elevation_drop_shadow() {
    // elevation の色を一意（赤）にして tuning から上書きすると、既存の box-shadow
    // lowering 経由でパネル背面に赤い影レイヤが描かれる。
    let (mut tree, _v, _t) = selectable_paragraph();
    let mut ct = ChromeTuning::default();
    ct.toolbar_shadow_color = [1.0, 0.0, 0.0, 0.8];
    tree.set_chrome_tuning(ct);
    select_a_range(&mut tree);
    tree.render(0.0);

    let bounds = tree.selection_toolbar().expect("a toolbar").bounds;
    let rects = fill_rects(&tree);
    // 影色（赤・RGB 保存、α は blur 減衰で変わる）の塗り。
    let shadow_idx = rects
        .iter()
        .find(|(_, _, _, _, _, c, _)| c[0] > 0.5 && c[1] < 0.1 && c[2] < 0.1 && c[3] > 0.0)
        .map(|t| t.0)
        .expect("a drop-shadow fill layer in the toolbar's shadow color");
    // パネル本体（bounds 位置・角丸）。
    let panel_idx = rects
        .iter()
        .find(|(_, x, y, w, h, _, r)| {
            (*x - bounds.x).abs() < 0.5
                && (*y - bounds.y).abs() < 0.5
                && (*w - bounds.width).abs() < 0.5
                && (*h - bounds.height).abs() < 0.5
                && *r > 0.0
        })
        .map(|t| t.0)
        .expect("the toolbar panel rect");
    // 影はパネルより先に（背面に）描かれる。
    assert!(shadow_idx < panel_idx, "the drop shadow is painted behind the panel");
}

#[test]
fn material_dividers_are_drawn_between_buttons() {
    // ディバイダ色を一意（緑）にして上書きし、ボタン境界に細い緑の線が出ることを見る。
    let (mut tree, _v, _t) = selectable_paragraph();
    let mut ct = ChromeTuning::default();
    ct.toolbar_divider_color = [0.0, 1.0, 0.0, 1.0];
    ct.toolbar_divider_width = 2.0;
    tree.set_chrome_tuning(ct);
    select_a_range(&mut tree);
    tree.render(0.0);

    let tb = tree.selection_toolbar().expect("a toolbar");
    // 読み取り専用は Copy / Select All の2ボタン → 境界は SelectAll の左端。
    let boundary = tb.buttons[1].bounds.x;
    let divider = fill_rects(&tree).into_iter().find(|(_, x, _, w, h, c, _)| {
        c[1] > 0.5
            && c[0] < 0.1
            && c[2] < 0.1
            && (*w - 2.0).abs() < 0.5
            && (*h - tb.bounds.height).abs() < 0.5
            && (*x - (boundary - 1.0)).abs() < 0.6
    });
    assert!(divider.is_some(), "a thin divider is drawn at the button boundary");
}

#[test]
fn toolbar_visual_height_comes_from_tuning() {
    // 高さを tuning で上書きすると、描かれるパネル矩形の高さがそれに追従する
    // （視覚値が再ビルド不要で tuning 由来であることを draw-op で固定）。
    let (mut tree, _v, _t) = selectable_paragraph();
    let mut ct = ChromeTuning::default();
    ct.toolbar_height = 60.0;
    tree.set_chrome_tuning(ct);
    select_a_range(&mut tree);
    tree.render(0.0);

    let bounds = tree.selection_toolbar().expect("a toolbar").bounds;
    assert_eq!(bounds.height, 60.0, "the laid-out toolbar uses the tuned height");
    let panel = fill_rects(&tree).into_iter().find(|(_, x, y, _, h, _, r)| {
        (*x - bounds.x).abs() < 0.5 && (*y - bounds.y).abs() < 0.5 && *r > 0.0 && (*h - 60.0).abs() < 0.5
    });
    assert!(panel.is_some(), "the drawn panel honors the tuned height");
}

#[test]
fn a_too_wide_action_set_folds_into_an_overflow_menu_and_taps_route_through_it() {
    // text-input は Cut/Copy/Paste/Select All の4アクション。viewport 200px には
    // 収まらないので末尾（Select All）が ⋮ オーバーフローへ畳まれる。
    let (mut tree, input) = text_input_with("hello world");
    tree.on_pointer_down_with_kind(2.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_move(60.0, 20.0);
    tree.on_pointer_up(60.0, 20.0);

    let tb = tree.selection_toolbar().expect("a toolbar over the editable selection");
    let overflow = tb.overflow.clone().expect("the bar overflows the narrow viewport");
    assert!(!overflow.open, "the submenu starts closed");
    // 畳まれていても全アクションは順序どおり提供される。
    assert_eq!(
        tb.actions(),
        vec![
            ToolbarAction::Cut,
            ToolbarAction::Copy,
            ToolbarAction::Paste,
            ToolbarAction::SelectAll,
        ],
    );
    assert!(
        overflow.items.iter().any(|b| b.action == ToolbarAction::SelectAll),
        "Select All is folded into the overflow menu",
    );

    // ⋮ トグルを押すと副メニューが開く（選択は触らない）。
    let toggle = overflow.toggle;
    tree.on_pointer_down(toggle.x + toggle.width / 2.0, toggle.y + toggle.height / 2.0);
    tree.on_pointer_up(toggle.x + toggle.width / 2.0, toggle.y + toggle.height / 2.0);
    assert!(tree.element_text_selection(input).is_some(), "tapping ⋮ keeps the selection");

    let opened = tree.selection_toolbar().expect("toolbar still showing");
    let opened_overflow = opened.overflow.expect("still overflowing");
    assert!(opened_overflow.open, "the ⋮ tap opened the submenu");

    // 展開した副メニューの Select All 項目を押すと、ヒットテストが当たって発火する。
    let item = opened_overflow
        .items
        .iter()
        .find(|b| b.action == ToolbarAction::SelectAll)
        .expect("Select All is a submenu item")
        .bounds;
    tree.on_pointer_down(item.x + item.width / 2.0, item.y + item.height / 2.0);

    let (s, e) = tree.element_text_selection(input).expect("a selection");
    let content = tree.element_get_text_content(input);
    assert_eq!((s, e), (0, content.len()), "the folded Select All ran from the submenu");
}

#[test]
fn the_overflow_submenu_panel_is_drawn_only_when_open() {
    let (mut tree, _input) = text_input_with("hello world");
    tree.on_pointer_down_with_kind(2.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_move(60.0, 20.0);
    tree.on_pointer_up(60.0, 20.0);
    tree.render(0.0);

    let tb = tree.selection_toolbar().expect("a toolbar");
    let panel = tb.overflow.clone().expect("overflowing").panel;
    let panel_drawn = |tree: &ElementTree| {
        fill_rects(tree).into_iter().any(|(_, x, y, w, h, _, _)| {
            (x - panel.x).abs() < 0.5
                && (y - panel.y).abs() < 0.5
                && (w - panel.width).abs() < 0.5
                && (h - panel.height).abs() < 0.5
        })
    };
    assert!(!panel_drawn(&tree), "closed: no submenu panel is drawn");

    // ⋮ を開く。
    let toggle = tb.overflow.unwrap().toggle;
    tree.on_pointer_down(toggle.x + toggle.width / 2.0, toggle.y + toggle.height / 2.0);
    tree.on_pointer_up(toggle.x + toggle.width / 2.0, toggle.y + toggle.height / 2.0);
    tree.render(0.0);
    assert!(panel_drawn(&tree), "open: the submenu panel is drawn");
}

#[test]
fn tapping_paste_replaces_the_editable_range_with_clipboard_text() {
    let (mut tree, input) = text_input_with("hello world");
    let clipboard = FakeClipboard::default();
    *clipboard.read.borrow_mut() = Some("X".to_string());
    tree.set_clipboard(Box::new(clipboard.clone()));

    // 先頭の非空範囲をタッチでドラッグ選択し、その上に Paste する。
    tree.on_pointer_down_with_kind(2.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_move(60.0, 20.0);
    tree.on_pointer_up(60.0, 20.0);
    let content = tree.element_get_text_content(input);
    let (s, e) = tree.element_text_selection(input).expect("a non-empty range");
    let mut expected = content.clone();
    expected.replace_range(s..e, "X");

    tap(&mut tree, ToolbarAction::Paste);

    assert_eq!(
        tree.element_get_text_content(input),
        expected,
        "paste replaces the selected range with the clipboard text",
    );
}
