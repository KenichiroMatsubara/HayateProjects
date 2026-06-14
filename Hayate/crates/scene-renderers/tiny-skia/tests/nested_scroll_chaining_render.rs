//! Pixel regression for the "nested scroll (chaining)" CSS Gallery sample
//! (issue #200). An inner `scroll-view` nested inside an outer `scroll-view`
//! must clip its overflowing content to its own box — both at rest and after
//! the outer scroll-view is scrolled, so the inner `Clip` correctly tracks the
//! outer scroll-offset `Group` transform.
//!
//! Layout (viewport space, scale 1.0), outer scroll-view at the origin:
//!
//! ```text
//!   outer scroll-view  (180 x 120)
//!   └─ column          (flex-direction: column)
//!      ├─ inner scroll-view (160 x 60)   ── screen y 0..60
//!      │  └─ green content  (160 x 200)  ── clipped to the inner box
//!      ├─ spacer            (160 x 20)   ── screen y 60..80  (transparent gap)
//!      └─ blue tail         (160 x 100)  ── screen y 80..180 (clipped by outer)
//! ```
//!
//! The inner green content is 200px tall but lives in a 60px inner box, so the
//! pre-#199 bug painted "Inner D"/"Inner E" past the inner box, on top of the
//! "Outer tail" rows. The transparent spacer gives a region where bleed is
//! detectable independent of sibling paint order.

use hayate_core::{
    Color, Dimension, ElementId, ElementKind, ElementTree, FlexDirectionValue, StyleProp,
};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn is_green(p: [u8; 4]) -> bool {
    p[1] > 200 && p[0] < 60 && p[2] < 60
}

fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 60 && p[1] < 60
}

/// Build the nested-scroll-chaining tree and return `(tree, outer_scroll_id)`.
fn nested_scroll_chaining_tree() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let column = tree.element_create(2, ElementKind::View);
    let inner = tree.element_create(3, ElementKind::ScrollView);
    let green = tree.element_create(4, ElementKind::View);
    let spacer = tree.element_create(5, ElementKind::View);
    let tail = tree.element_create(6, ElementKind::View);

    tree.set_root(outer);
    tree.set_viewport(200.0, 200.0);

    tree.element_append_child(outer, column);
    tree.element_append_child(column, inner);
    tree.element_append_child(inner, green);
    tree.element_append_child(column, spacer);
    tree.element_append_child(column, tail);

    tree.element_set_style(
        outer,
        &[
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(180.0)),
            StyleProp::Height(Dimension::px(120.0)),
        ],
    );
    tree.element_set_style(
        column,
        &[
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(160.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(60.0)),
        ],
    );
    tree.element_set_style(
        green,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_set_style(
        spacer,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(20.0)),
        ],
    );
    tree.element_set_style(
        tail,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(BLUE),
        ],
    );

    tree.render(0.0);
    (tree, outer)
}

/// At rest, the inner scroll-view's overflowing content is clipped to its own
/// 60px box and does not bleed onto the transparent gap or the outer tail.
#[test]
fn inner_content_clipped_to_its_box_at_rest() {
    let (tree, _outer) = nested_scroll_chaining_tree();
    let mut pixmap = Pixmap::new(200, 200).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);

    // Inner content visible inside the 60px inner box.
    assert!(
        is_green(pixel(&pixmap, 80, 30)),
        "inner content should be visible inside its box, got {:?}",
        pixel(&pixmap, 80, 30)
    );
    // Transparent gap below the inner box (y 60..80): inner content must not
    // bleed here. This is the "Inner D/Inner E" overlap region.
    assert_eq!(
        pixel(&pixmap, 80, 70),
        [255, 255, 255, 255],
        "inner content must be clipped to its box, not bleed onto the gap"
    );
    // Outer tail below the gap (y 80..120) is the blue tail, undisturbed.
    assert!(
        is_blue(pixel(&pixmap, 80, 100)),
        "outer tail should be visible and un-overlapped, got {:?}",
        pixel(&pixmap, 80, 100)
    );
}

/// After scrolling the outer scroll-view up by 30px, the inner `Clip` must
/// track the inner scroll-view's new painted position (shifted up by the outer
/// scroll-offset `Group` transform) instead of staying at its un-transformed
/// local coordinates. With the outer offset applied:
///
/// ```text
///   inner box  ── screen y -30..30   (green clipped to its bottom at y=30)
///   gap        ── screen y  30..50   (transparent)
///   blue tail  ── screen y  50..150  (clipped by outer to 50..120)
/// ```
///
/// If the inner clip drifted (clipped at un-transformed local y 0..60 while the
/// content paints shifted up), green would bleed down to ~y=30..60 and paint
/// over the gap.
#[test]
fn inner_clip_tracks_outer_scroll_offset_without_drift() {
    let (mut tree, outer) = nested_scroll_chaining_tree();
    tree.element_set_scroll_offset(outer, 0.0, 30.0);
    tree.render(0.0);

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);

    // Inner content still visible near the (now shifted) top of the inner box.
    assert!(
        is_green(pixel(&pixmap, 80, 15)),
        "inner content should remain visible after outer scroll, got {:?}",
        pixel(&pixmap, 80, 15)
    );
    // Gap region (screen y 30..50): the inner clip moved up with the content,
    // so green must stop at ~y=30 — no drift bleeding into the gap.
    assert_eq!(
        pixel(&pixmap, 80, 45),
        [255, 255, 255, 255],
        "inner clip must track the outer scroll transform (no drift past y=30)"
    );
    // Outer tail, now shifted up to screen y 50..120, is still blue.
    assert!(
        is_blue(pixel(&pixmap, 80, 70)),
        "outer tail should be visible after outer scroll, got {:?}",
        pixel(&pixmap, 80, 70)
    );
}
