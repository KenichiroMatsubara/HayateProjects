//! Scrollbar overlay chrome lowering (ADR-0110, #407). The first tracer bullet:
//! a scrollable `scroll-view` draws a Mouse/Pen-style thumb overlay under its
//! ScrollView anchor, on each overflowing axis, with the thumb geometry derived
//! from the Scroll Offset and content size. No Pointer Modality branch yet (the
//! Touch transient indicator is a later slice) and no layout space reserved.
//!
//! Exercised through the public `ElementTree` interface via both the
//! `RecordingPainter` DrawOp stream and a SceneGraph `NodeKind` walk (prior art:
//! `scroll_view_scene.rs`, `selection_chrome_modality.rs`).

use hayate_core::element::scene_build::{
    SCROLLBAR_THICKNESS, SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_OPACITY,
};
use hayate_core::{
    Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, NodeId, NodeKind,
    RecordingPainter, StyleProp, render_scene_graph,
};

/// Final composited thumb fill colour (RGB at the overlay opacity).
fn thumb_rgba() -> [f32; 4] {
    SCROLLBAR_THUMB_COLOR
        .with_opacity(SCROLLBAR_THUMB_OPACITY)
        .to_array_f32()
}

/// All scrollbar-thumb `Rect` nodes in the scene graph as `(id, x, y, w, h)`,
/// identified by the thumb fill colour.
fn thumb_nodes(tree: &ElementTree) -> Vec<(NodeId, f32, f32, f32, f32)> {
    let sg = tree.scene_graph();
    let rgba = thumb_rgba();
    sg.iter()
        .filter_map(|(id, n)| match &n.kind {
            NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                ..
            } if *color == rgba => Some((id, *x, *y, *width, *height)),
            _ => None,
        })
        .collect()
}

/// Thumb fill ops in the recorded DrawOp stream (public painter path).
fn thumb_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    let rgba = thumb_rgba();
    painter
        .ops()
        .iter()
        .filter(|op| matches!(op, DrawOp::FillRect { color, .. } if *color == rgba))
        .cloned()
        .collect()
}

/// Walk anchors upward: is `node` a descendant of the `ElementAnchor` for `scroll`?
fn is_under_scroll_anchor(tree: &ElementTree, node: NodeId, scroll: ElementId) -> bool {
    let sg = tree.scene_graph();
    let mut current = Some(node);
    while let Some(id) = current {
        if let Some(n) = sg.get(id) {
            if matches!(&n.kind, NodeKind::ElementAnchor { element_id } if *element_id == scroll) {
                return true;
            }
        }
        current = sg.parent_of(id);
    }
    false
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
fn scrollable_view_lowers_a_thumb_under_its_anchor() {
    let (tree, scroll) = vertical_overflow_scroll_view();

    let thumbs = thumb_nodes(&tree);
    assert_eq!(
        thumbs.len(),
        1,
        "a vertically-overflowing scroll-view draws exactly one (vertical) thumb"
    );
    let (thumb_id, _, _, width, _) = thumbs[0];
    assert!(
        is_under_scroll_anchor(&tree, thumb_id, scroll),
        "the thumb is lowered under the ScrollView anchor",
    );
    assert_eq!(
        width, SCROLLBAR_THICKNESS,
        "a vertical thumb is SCROLLBAR_THICKNESS wide",
    );
    assert_eq!(
        thumb_ops(&tree).len(),
        1,
        "the public painter records the thumb as one fill op",
    );
}

/// A `scroll-view` whose `content` is `cw × ch` inside a `bw × bh` box.
fn scroll_view_with_content(bw: f32, bh: f32, cw: f32, ch: f32) -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(bw)),
            StyleProp::Height(Dimension::px(bh)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(cw)),
            StyleProp::Height(Dimension::px(ch)),
            // Keep the explicit size: a flex item would otherwise shrink along the
            // main axis to fit the box, hiding the horizontal overflow under test.
            StyleProp::FlexShrink(0.0),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

#[test]
fn content_that_fits_draws_no_scrollbar() {
    // Content smaller than the box on both axes: nothing to scroll, no thumb.
    let (tree, _scroll) = scroll_view_with_content(200.0, 200.0, 100.0, 100.0);
    assert!(
        thumb_nodes(&tree).is_empty(),
        "a scroll-view whose content fits draws no scrollbar",
    );
    assert!(
        thumb_ops(&tree).is_empty(),
        "and the painter records no thumb fill",
    );
}

#[test]
fn only_the_overflowing_axis_is_drawn() {
    // Content overflows the width but fits the height: a single horizontal thumb
    // (height == thickness, sitting at the bottom edge), no vertical thumb.
    let (tree, scroll) = scroll_view_with_content(100.0, 100.0, 300.0, 100.0);
    let thumbs = thumb_nodes(&tree);
    assert_eq!(thumbs.len(), 1, "only the overflowing (horizontal) axis is drawn");

    let (_, _, ty, tw, th) = thumbs[0];
    assert_eq!(th, SCROLLBAR_THICKNESS, "a horizontal thumb is THICKNESS tall");
    assert!(tw > th, "and runs along the horizontal axis");
    let (_, sy, _, sh) = tree.element_layout_rect(scroll).unwrap();
    assert!(
        ty + th <= sy + sh + 0.01,
        "the horizontal thumb sits at the bottom edge of the box",
    );
}

#[test]
fn both_axes_overflow_draws_two_thumbs() {
    // Content overflows both axes: a vertical thumb (THICKNESS wide) at the right
    // and a horizontal thumb (THICKNESS tall) at the bottom.
    let (tree, _scroll) = scroll_view_with_content(100.0, 100.0, 300.0, 300.0);
    let thumbs = thumb_nodes(&tree);
    assert_eq!(thumbs.len(), 2, "both overflowing axes are drawn");

    let vertical = thumbs.iter().filter(|t| t.3 == SCROLLBAR_THICKNESS).count();
    let horizontal = thumbs.iter().filter(|t| t.4 == SCROLLBAR_THICKNESS).count();
    assert_eq!(vertical, 1, "exactly one vertical thumb");
    assert_eq!(horizontal, 1, "exactly one horizontal thumb");
}

fn has_ancestor(tree: &ElementTree, node: NodeId, pred: impl Fn(&NodeKind) -> bool) -> bool {
    let sg = tree.scene_graph();
    let mut current = Some(node);
    while let Some(id) = current {
        if let Some(n) = sg.get(id) {
            if pred(&n.kind) {
                return true;
            }
        }
        current = sg.parent_of(id);
    }
    false
}

#[test]
fn nested_inner_thumb_is_anchored_and_clipped_inside_the_outer_box() {
    // Outer 200×100 (fits) holding an inner 180×80 scroll-view whose 180×300
    // content overflows vertically. Only the inner draws a thumb, and that thumb
    // hangs under the inner ScrollView anchor — which itself nests under the outer
    // ScrollView's Clip — so the inner thumb tracks the inner box and is bounded
    // by the outer box (it cannot leak outside it; #199/#200 coordinate system).
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let inner = tree.element_create(2, ElementKind::ScrollView);
    let content = tree.element_create(3, ElementKind::View);
    tree.set_root(outer);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(180.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(180.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, content);
    tree.render(0.0);

    let thumbs = thumb_nodes(&tree);
    assert_eq!(thumbs.len(), 1, "only the overflowing inner scroll-view draws a thumb");
    let inner_thumb = thumbs[0].0;

    assert!(
        is_under_scroll_anchor(&tree, inner_thumb, inner),
        "the inner thumb is lowered under the inner ScrollView anchor",
    );
    assert!(
        is_under_scroll_anchor(&tree, inner_thumb, outer),
        "the inner thumb nests under the outer ScrollView anchor too",
    );
    assert!(
        has_ancestor(&tree, inner_thumb, |k| matches!(k, NodeKind::Clip { .. })),
        "the inner thumb is clipped by the outer box and cannot leak outside it",
    );
}

#[test]
fn thumb_tracks_the_scroll_offset() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let at_top = thumb_nodes(&tree)[0].2;

    tree.element_set_scroll_offset(scroll, 0.0, 200.0);
    tree.render(0.0);
    let scrolled = thumb_nodes(&tree)[0].2;

    assert!(
        scrolled > at_top,
        "scrolling down moves the thumb down the track (top {at_top} -> {scrolled})",
    );

    // Scrolled fully to the end, the thumb's bottom must reach the track's end —
    // its geometry follows the offset as a fraction of the scrollable range.
    let (_, _, ty, _, th) = thumb_nodes(&tree)[0];
    let (_, sy, _, sh) = tree.element_layout_rect(scroll).unwrap();
    assert!(
        (ty + th) <= sy + sh + 0.01 && (ty + th) >= sy + sh - 4.0,
        "at max offset the thumb sits at the bottom of the track",
    );
}
