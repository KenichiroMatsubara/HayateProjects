//! Material のフローティング選択ツールバー（core が描く選択クローム、ADR-0097）。
//! ツールバーのアクション・ジオメトリ・ヒットテスト・クリップボード配線を公開
//! `ElementTree` インターフェース越しに検証する。

use std::cell::RefCell;
use std::rc::Rc;

use hayate_core::{
    Clipboard, Dimension, DrawOp, ElementId, ElementKind, ElementTree, PointerKind,
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
