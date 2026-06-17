//! ADR-0104 / #364: the edit-selection highlight is focus-linked, and the blur
//! lifecycle is PointerKind-dependent — Mouse/Pen remember the range (Chromium
//! form-control parity), Touch collapses it to a caret (Android behaviour).

use hayate_core::{
    Dimension, DrawOp, ElementId, ElementKind, ElementTree, FlexDirectionValue, PointerKind,
    RecordingPainter, StyleProp, render_scene_graph,
};

/// Material selection tint (ADR-0097): the colour of an edit-selection highlight.
const HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// Whether the rendered scene contains a selection-highlight rect.
fn has_highlight(tree: &ElementTree) -> bool {
    !highlight_bands(tree).is_empty()
}

/// The vertical bands (y_min, y_max) of every selection-highlight rect.
fn highlight_bands(tree: &ElementTree) -> Vec<(f32, f32)> {
    draw_ops(tree)
        .iter()
        .filter_map(|op| match op {
            DrawOp::FillRect { y, height, color, .. } if *color == HIGHLIGHT_COLOR => {
                Some((*y, *y + *height))
            }
            _ => None,
        })
        .collect()
}

/// A column root holding a focused text-input on top and an empty `pad` view
/// below it to tap for blurring. Returns (tree, input, pad). Both laid out.
fn input_with_outside(content: &str) -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let input = tree.element_create(2, ElementKind::TextInput);
    let pad = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_set_style(
        pad,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(120.0)),
        ],
    );
    tree.element_append_child(root, input);
    tree.element_append_child(root, pad);
    tree.element_append_text_content(input, content);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input, pad)
}

/// Drag-select a range inside the top input (its content sits on the y≈20 row).
fn drag_select(tree: &mut ElementTree) {
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(70.0, 20.0);
}

#[test]
fn highlight_is_drawn_only_while_the_input_is_focused() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    assert!(tree.element_text_selection(input).is_some(), "a range is selected");
    assert!(
        has_highlight(&tree),
        "the focused input paints its selection highlight",
    );

    // Blur the field without collapsing the range: an unfocused text-input must
    // not paint an active selection highlight, even though the range is still
    // held in its EditState (ADR-0104, focus-linked highlight).
    tree.element_blur(input);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "the range is still remembered after blur",
    );
    assert!(
        !has_highlight(&tree),
        "an unfocused text-input draws no selection highlight",
    );
}

#[test]
fn touch_blur_collapses_the_selection_and_dismisses_chrome() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    assert!(tree.element_text_selection(input).is_some(), "a range is selected");
    assert!(tree.selection_toolbar().is_some(), "the selection shows chrome");

    // Tapping outside the field with a Touch pointer (Android behaviour, #364):
    // the edit selection collapses to a caret and the selection chrome is gone.
    tree.on_pointer_down_with_kind(100.0, 100.0, 0, PointerKind::Touch);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_none(),
        "a Touch blur collapses the edit selection to a caret",
    );
    assert!(
        tree.element_caret_byte_index(input).is_some(),
        "the caret survives the collapse",
    );
    assert!(
        tree.selection_toolbar().is_none(),
        "the selection chrome is dismissed after a Touch blur",
    );
}

#[test]
fn mouse_blur_remembers_the_range_and_refocus_restores_the_highlight() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    let range = tree.element_text_selection(input).expect("a range is selected");

    // Tapping outside with a Mouse pointer blurs the field but keeps the range
    // (Chromium form-control parity, #364): the highlight hides while unfocused.
    tree.on_pointer_down_with_kind(100.0, 100.0, 0, PointerKind::Mouse);
    tree.render(0.0);
    assert_eq!(
        tree.element_text_selection(input),
        Some(range),
        "a Mouse blur remembers the selected range",
    );
    assert!(!has_highlight(&tree), "the highlight hides while unfocused");

    // Returning focus to the field (e.g. Tab back) re-lights the remembered
    // range — the focus-linked highlight reappears unchanged.
    tree.element_focus(input);
    tree.render(0.0);
    assert_eq!(
        tree.element_text_selection(input),
        Some(range),
        "the range is unchanged on refocus",
    );
    assert!(has_highlight(&tree), "refocusing restores the selection highlight");
}

#[test]
fn unfocused_input_shows_no_chrome_even_when_it_remembers_a_range() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    assert!(tree.selection_toolbar().is_some(), "the focused selection shows chrome");

    // A Mouse blur keeps the range but hides the chrome (active = focused,
    // ADR-0104): the toolbar must not linger over an unfocused field.
    tree.on_pointer_down_with_kind(100.0, 100.0, 0, PointerKind::Mouse);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "the range is still remembered",
    );
    assert!(
        tree.selection_toolbar().is_none(),
        "an unfocused text-input shows no selection chrome",
    );

    // Refocusing brings the chrome back with the remembered range.
    tree.element_focus(input);
    tree.render(0.0);
    assert!(tree.selection_toolbar().is_some(), "refocus restores the chrome");
}

/// A column of two text-inputs (each 40px tall) separated by an 80px spacer on a
/// 200×240 viewport — top row y≈[0,40], bottom row y≈[120,160]. The spacer keeps
/// the top selection's floating toolbar clear of the bottom input's hit area
/// (ADR-0097, #272). Returns (tree, top, bottom), both laid out, top focused.
fn two_inputs() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let top = tree.element_create(2, ElementKind::TextInput);
    let spacer = tree.element_create(3, ElementKind::View);
    let bottom = tree.element_create(4, ElementKind::TextInput);
    tree.set_root(root);
    tree.set_viewport(200.0, 240.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(240.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    for &inp in &[top, bottom] {
        tree.element_set_style(
            inp,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(40.0)),
                StyleProp::FontSize(16.0),
            ],
        );
    }
    tree.element_set_style(
        spacer,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(80.0)),
        ],
    );
    tree.element_append_child(root, top);
    tree.element_append_child(root, spacer);
    tree.element_append_child(root, bottom);
    tree.element_append_text_content(top, "hello world");
    tree.element_append_text_content(bottom, "hello world");
    tree.element_focus(top);
    tree.render(0.0);
    (tree, top, bottom)
}

#[test]
fn switching_text_inputs_never_lights_two_at_once() {
    let (mut tree, top, bottom) = two_inputs();

    // Select a range in the top input (its row is y≈[0,40]).
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(70.0, 20.0);
    tree.render(0.0);
    assert!(tree.element_text_selection(top).is_some(), "top has a range");
    let bands = highlight_bands(&tree);
    assert!(!bands.is_empty(), "the top input is highlighted");
    assert!(
        bands.iter().all(|&(_, y1)| y1 <= 40.0),
        "only the top row lights up, got {bands:?}",
    );

    // Drag-select in the bottom input (row y≈[120,160]). Focus moves to it; the
    // top input's highlight must not linger alongside the bottom's (single
    // active = focused, ADR-0104).
    tree.on_pointer_down(2.0, 140.0);
    tree.on_pointer_move(70.0, 140.0);
    tree.render(0.0);
    assert!(tree.element_text_selection(bottom).is_some(), "bottom has a range");
    let bands = highlight_bands(&tree);
    assert!(!bands.is_empty(), "the bottom input is highlighted");
    assert!(
        bands.iter().all(|&(y0, y1)| y0 > 40.0 && y1 > 40.0),
        "no highlight lingers over the (now unfocused) top input row [0,40], got {bands:?}",
    );
}
