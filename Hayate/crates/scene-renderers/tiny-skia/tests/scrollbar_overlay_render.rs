//! Pixel regression: the scrollbar overlay thumb (ADR-0110, #407) is rasterised
//! at the box edge over the content, tracks the Scroll Offset, and — for a nested
//! scroll-view — stays clipped inside the outer box. Mirrors the S4 prior art
//! `scroll_view_render.rs` / `nested_scroll_chaining_render.rs`.

use hayate_core::{Color, Dimension, ElementId, ElementKind, ElementTree, StyleProp};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn render(tree: &ElementTree, w: u32, h: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(w, h).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);
    pixmap
}

/// Pure green content, untouched by any overlay: g high, r/b ~0.
fn is_plain_green(p: [u8; 4]) -> bool {
    p[1] > 230 && p[0] < 40 && p[2] < 40
}

/// Green darkened by the translucent dark thumb composited over it: still green-
/// dominant but visibly dimmer than plain content.
fn is_thumb_over_green(p: [u8; 4]) -> bool {
    p[1] > 100 && p[1] < 210 && p[0] < 40 && p[2] < 40
}

fn vertical_scroll_view() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(200.0, 200.0);
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
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

#[test]
fn thumb_is_painted_at_the_right_edge_over_content() {
    let (tree, _scroll) = vertical_scroll_view();
    let pixmap = render(&tree, 120, 120);

    // Right edge near the top: the thumb darkens the green content here.
    assert!(
        is_thumb_over_green(pixel(&pixmap, 95, 12)),
        "thumb should darken the content at the right edge, got {:?}",
        pixel(&pixmap, 95, 12)
    );
    // Left of the thumb is plain, full-brightness green content.
    assert!(
        is_plain_green(pixel(&pixmap, 40, 12)),
        "content away from the thumb stays plain green, got {:?}",
        pixel(&pixmap, 40, 12)
    );
}

#[test]
fn thumb_tracks_the_offset_in_pixels() {
    let (mut tree, scroll) = vertical_scroll_view();

    // At rest the thumb is near the top; the bottom of the track is plain green.
    let at_rest = render(&tree, 120, 120);
    assert!(
        is_thumb_over_green(pixel(&at_rest, 95, 12)),
        "thumb starts near the top of the track, got {:?}",
        pixel(&at_rest, 95, 12)
    );
    assert!(
        is_plain_green(pixel(&at_rest, 95, 90)),
        "the bottom of the track is clear at rest, got {:?}",
        pixel(&at_rest, 95, 90)
    );

    // Scrolled to the end the thumb slides to the bottom; the top clears.
    tree.element_set_scroll_offset(scroll, 0.0, 200.0);
    tree.render(0.0);
    let scrolled = render(&tree, 120, 120);
    assert!(
        is_thumb_over_green(pixel(&scrolled, 95, 90)),
        "thumb reaches the bottom after scrolling, got {:?}",
        pixel(&scrolled, 95, 90)
    );
    assert!(
        is_plain_green(pixel(&scrolled, 95, 12)),
        "the top of the track clears after scrolling, got {:?}",
        pixel(&scrolled, 95, 12)
    );
}

fn is_white(p: [u8; 4]) -> bool {
    p == [255, 255, 255, 255]
}

#[test]
fn nested_inner_thumb_stays_inside_the_inner_box() {
    // Outer 140×120 holding an inner 100×80 scroll-view (green 100×300 content)
    // at the top-left. The inner thumb must darken the content at the inner box's
    // right edge but never paint into the empty outer space below or beside the
    // inner box — it is clipped to (and tracks) the inner box.
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let inner = tree.element_create(2, ElementKind::ScrollView);
    let content = tree.element_create(3, ElementKind::View);
    tree.set_root(outer);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(140.0)),
            StyleProp::Height(Dimension::px(120.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, content);
    tree.render(0.0);
    let pixmap = render(&tree, 160, 140);

    // Inner thumb darkens the content at the inner box's right edge (x≈92..98).
    assert!(
        is_thumb_over_green(pixel(&pixmap, 94, 12)),
        "inner thumb paints at the inner box right edge, got {:?}",
        pixel(&pixmap, 94, 12)
    );
    // Below the inner box (y 80..120) is empty outer space: no thumb leak.
    assert!(
        is_white(pixel(&pixmap, 94, 100)),
        "inner thumb must not leak below the inner box, got {:?}",
        pixel(&pixmap, 94, 100)
    );
    // Right of the inner box (x 100..140) is empty outer space: no thumb leak.
    assert!(
        is_white(pixel(&pixmap, 120, 40)),
        "inner thumb must not leak beside the inner box, got {:?}",
        pixel(&pixmap, 120, 40)
    );
}

#[test]
fn fitting_content_paints_no_thumb() {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(200.0, 200.0);
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
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    let pixmap = render(&tree, 120, 120);

    // Every column, including the right edge, is plain green — no thumb overlay.
    assert!(
        is_plain_green(pixel(&pixmap, 95, 50)),
        "a fitting scroll-view paints no thumb at the edge, got {:?}",
        pixel(&pixmap, 95, 50)
    );
}
