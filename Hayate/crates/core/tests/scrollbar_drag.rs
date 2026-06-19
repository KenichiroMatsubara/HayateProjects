//! Mouse/Pen scrollbar operation (#409, ADR-0110, SCR-04): the thumb drawn by
//! #407 is now operable. A pointer-down on the thumb + drag moves the Scroll
//! Offset continuously; a click on the track margin pages by one named step; a
//! thumb drag that reaches the axis end chains the remainder to the ancestor
//! ScrollView through the same `apply_wheel_delta` seam as the wheel (scroll
//! chaining parity, ADR-0084). Operation-derived offset changes converge on the
//! same Scroll Offset seam (`element_set_scroll_offset`, ADR-0046) the wheel
//! path commits to.
//!
//! Driven through the public pointer interface (`on_pointer_down_with_kind` +
//! `on_pointer_move`), reading the thumb's drawn geometry back out of the scene
//! graph — prior art: `selection_handles.rs`, `scroll_view_scene.rs`,
//! `scrollbar_overlay_scene.rs`.

use hayate_core::element::pointer::PointerKind;
use hayate_core::element::scene_build::{
    SCROLLBAR_THICKNESS, SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_OPACITY,
};
use hayate_core::{Color, Dimension, ElementId, ElementKind, ElementTree, NodeKind, StyleProp};

/// Final composited thumb fill colour (RGB at the overlay opacity).
fn thumb_rgba() -> [f32; 4] {
    SCROLLBAR_THUMB_COLOR
        .with_opacity(SCROLLBAR_THUMB_OPACITY)
        .to_array_f32()
}

/// Every vertical scrollbar thumb rect `(x, y, w, h)` in canvas coords, found by
/// the thumb fill colour and its THICKNESS cross-axis width.
fn vertical_thumbs(tree: &ElementTree) -> Vec<(f32, f32, f32, f32)> {
    let rgba = thumb_rgba();
    tree.scene_graph()
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                ..
            } if *color == rgba && (*width - SCROLLBAR_THICKNESS).abs() < 0.01 => {
                Some((*x, *y, *width, *height))
            }
            _ => None,
        })
        .collect()
}

/// The single vertical thumb, asserting there is exactly one.
fn vertical_thumb(tree: &ElementTree) -> (f32, f32, f32, f32) {
    let thumbs = vertical_thumbs(tree);
    assert_eq!(thumbs.len(), 1, "expected exactly one vertical thumb");
    thumbs[0]
}

/// A `scroll-view` whose content overflows only the vertical axis: a 100×100 box
/// holding 100×300 content. Returns `(tree, scroll_id)`.
fn vertical_overflow_scroll_view() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

#[test]
fn dragging_the_thumb_scrolls_continuously() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let (tx, ty, tw, th) = vertical_thumb(&tree);
    let (cx, cy) = (tx + tw / 2.0, ty + th / 2.0);

    // Grab the thumb under Mouse modality. The press alone does not scroll.
    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Mouse);
    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        0.0,
        "pressing the thumb does not by itself move the offset",
    );

    // Dragging the thumb down increases the vertical Scroll Offset.
    tree.on_pointer_move(cx, cy + 10.0);
    let after_first = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_first > 0.0,
        "dragging the thumb down moves the scroll offset (got {after_first})",
    );

    // Continued drag keeps moving it — the offset tracks the pointer continuously.
    tree.on_pointer_move(cx, cy + 20.0);
    let after_second = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_second > after_first,
        "a continued drag keeps moving the offset ({after_first} -> {after_second})",
    );

    // Releasing ends the drag: a later move no longer tracks the thumb.
    tree.on_pointer_up(cx, cy + 20.0);
    tree.on_pointer_move(cx, cy + 40.0);
    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        after_second,
        "after release the thumb no longer follows the pointer",
    );
}

#[test]
fn clicking_the_track_pages_the_offset() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let (tx, ty, tw, th) = vertical_thumb(&tree);

    // A press on the track *below* the thumb pages the offset forward (toward the
    // end), without grabbing a thumb.
    let track_x = tx + tw / 2.0;
    tree.on_pointer_down_with_kind(track_x, ty + th + 20.0, 0, PointerKind::Mouse);
    let after_forward = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_forward > 0.0,
        "a track press below the thumb pages the offset forward (got {after_forward})",
    );
    tree.on_pointer_up(track_x, ty + th + 20.0);

    // Re-render so the thumb sits at its new position, then press *above* it to
    // page back toward the start.
    tree.render(0.0);
    let (_, aty, _, _) = vertical_thumb(&tree);
    tree.on_pointer_down_with_kind(track_x, aty / 2.0, 0, PointerKind::Mouse);
    let after_back = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_back < after_forward,
        "a track press above the thumb pages the offset back ({after_forward} -> {after_back})",
    );
}

/// Nested scroll-views (prior art: `document_runtime::nested_scroll_tree`): an
/// outer 200×200 holding a scrollable inner 200×100 (its 200×300 leaf overflows)
/// plus a 200×250 tail, so the outer overflows vertically too. Returns
/// `(tree, outer, inner)`.
fn nested_scroll_tree() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let inner = tree.element_create(2, ElementKind::ScrollView);
    let leaf = tree.element_create(3, ElementKind::View);
    let tail = tree.element_create(4, ElementKind::View);
    tree.set_root(outer);
    tree.set_viewport(400.0, 400.0);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, leaf);
    tree.element_append_child(outer, tail);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_set_style(
        leaf,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_style(
        tail,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(250.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.render(0.0);
    (tree, outer, inner)
}

#[test]
fn thumb_drag_chains_to_the_ancestor_at_the_inner_end() {
    let (mut tree, outer, inner) = nested_scroll_tree();

    // The inner thumb is the shorter of the two vertical thumbs (its content
    // overflows more, so its thumb is smaller); grab its centre.
    let mut thumbs = vertical_thumbs(&tree);
    assert_eq!(
        thumbs.len(),
        2,
        "both inner and outer draw a vertical thumb"
    );
    thumbs.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    let (tx, ty, tw, th) = thumbs[0];
    let inner_max = tree.element_scroll_max_offset(inner).1;
    assert!(inner_max > 0.0 && tree.element_scroll_max_offset(outer).1 > 0.0);

    let cx = tx + tw / 2.0;
    let cy = ty + th / 2.0;
    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Mouse);

    // Drag far enough past the inner thumb's travel to overrun the inner's range.
    tree.on_pointer_move(cx, cy + th + 200.0);

    let inner_y = tree.element_get_scroll_offset(inner).1;
    let outer_y = tree.element_get_scroll_offset(outer).1;
    assert!(
        (inner_y - inner_max).abs() < 1e-3,
        "the inner offset is pinned at its max ({inner_y} vs {inner_max})",
    );
    assert!(
        outer_y > 0.0,
        "the remaining drag chains to the ancestor ScrollView (outer={outer_y})",
    );
}

#[test]
fn touch_press_does_not_operate_the_scrollbar() {
    // The Mouse/Pen scrollbar is interactive; Touch gets a non-interactive
    // transient indicator instead (ADR-0110). A touch press on the thumb's pixels
    // therefore neither grabs nor scrolls — it falls through to the content.
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let (tx, ty, tw, th) = vertical_thumb(&tree);
    let (cx, cy) = (tx + tw / 2.0, ty + th / 2.0);

    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Touch);
    tree.on_pointer_move(cx, cy + 30.0);

    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        0.0,
        "a Touch press does not drag the thumb (no interactive Mouse/Pen bar)",
    );
}
