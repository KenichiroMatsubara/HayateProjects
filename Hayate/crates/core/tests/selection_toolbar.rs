//! Material floating selection toolbar — core-drawn selection chrome
//! (ADR-0097, issue #272). The toolbar's actions, geometry, hit-testing and
//! clipboard wiring are exercised through the public `ElementTree` interface.

use std::cell::RefCell;
use std::rc::Rc;

use hayate_core::{
    Clipboard, Dimension, DrawOp, ElementId, ElementKind, ElementTree, RecordingPainter, StyleProp,
    ToolbarAction, render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// A `Clipboard` double that records writes and serves a preset read value, so a
/// test can assert what the toolbar pushed/pulled across the adapter boundary.
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

/// Press the toolbar button for `action` at its center (its action fires on the
/// press). Panics when no toolbar or no such button is showing.
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

/// Build `<view [selectable]><text "Hello world"></view>` on one line and
/// return (tree, view, text). Mirrors the harness in `text_selection.rs`.
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

/// Drag-select a range, then release. Leaves a non-empty read-only selection.
fn select_a_range(tree: &mut ElementTree) {
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);
    tree.on_pointer_up(70.0, 8.0);
}

/// A laid-out, focused text-input carrying `content`. Returns (tree, input).
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

    // The toolbar's background panel is drawn as a filled rect at its bounds.
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

    // Click in empty space to clear the selection, then re-render.
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

    // Drag-select a range inside the field.
    tree.on_pointer_down(2.0, 20.0);
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
    // Copy leaves the selection in place — its toolbar is still showing.
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

    // Drag-select a non-empty leading range, then Cut.
    tree.on_pointer_down(2.0, 20.0);
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

    // Drag-select a non-empty leading range, then Paste over it.
    tree.on_pointer_down(2.0, 20.0);
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
