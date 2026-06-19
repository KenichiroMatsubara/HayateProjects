//! Pointer-Modality branch for scrollbar chrome (ADR-0110, SCR-04, #410). The
//! same modality axis that gates selection chrome (ADR-0104,
//! `selection_chrome_modality.rs`) splits the scrollbar overlay in two: Mouse/Pen
//! get the operable thumb (#407 + #409), Touch gets a non-operable transient
//! indicator that appears while scrolling and fades after it stops. The indicator
//! carries no thumb/track hit region — content flick scrolls, not a drag.
//!
//! Driven through the public `ElementTree` interface — the pointer wire
//! (`on_pointer_*_with_kind`), the Scroll Offset seam, and the rendered
//! SceneGraph — never the lowering internals (prior art:
//! `selection_chrome_modality.rs`, `scrollbar_overlay_scene.rs`).

use hayate_core::element::pointer::PointerKind;
use hayate_core::element::scene_build::{
    SCROLLBAR_INDICATOR_COLOR, SCROLLBAR_INDICATOR_FADE_MS, SCROLLBAR_INDICATOR_HOLD_MS,
    SCROLLBAR_INDICATOR_OPACITY, SCROLLBAR_INDICATOR_THICKNESS, SCROLLBAR_THICKNESS,
    SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_OPACITY,
};
use hayate_core::{Color, Dimension, ElementId, ElementKind, ElementTree, NodeKind, StyleProp};

/// Final composited operable-thumb fill colour (RGB at the overlay opacity).
fn thumb_rgba() -> [f32; 4] {
    SCROLLBAR_THUMB_COLOR
        .with_opacity(SCROLLBAR_THUMB_OPACITY)
        .to_array_f32()
}

/// Operable Mouse/Pen thumb rects `(x, y, w, h)`: the thumb fill colour at the
/// cross-axis THICKNESS that the #409 interactive bar paints.
fn operable_thumbs(tree: &ElementTree) -> Vec<(f32, f32, f32, f32)> {
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
            } if *color == rgba
                && ((*width - SCROLLBAR_THICKNESS).abs() < 0.01
                    || (*height - SCROLLBAR_THICKNESS).abs() < 0.01) =>
            {
                Some((*x, *y, *width, *height))
            }
            _ => None,
        })
        .collect()
}

/// Touch transient-indicator rects `(x, y, w, h)`: the indicator colour at the
/// cross-axis INDICATOR_THICKNESS, regardless of its (fading) opacity.
fn vertical_indicators(tree: &ElementTree) -> Vec<(f32, f32, f32, f32)> {
    let rgb = SCROLLBAR_INDICATOR_COLOR.to_array_f32();
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
            } if (color[0], color[1], color[2]) == (rgb[0], rgb[1], rgb[2])
                && color[3] > 0.0
                && (*width - SCROLLBAR_INDICATOR_THICKNESS).abs() < 0.01 =>
            {
                Some((*x, *y, *width, *height))
            }
            _ => None,
        })
        .collect()
}

/// The alpha of the (single) vertical indicator rect, or `None` if none is drawn.
fn indicator_alpha(tree: &ElementTree) -> Option<f32> {
    let rgb = SCROLLBAR_INDICATOR_COLOR.to_array_f32();
    tree.scene_graph().iter().find_map(|(_, n)| match &n.kind {
        NodeKind::Rect { width, color, .. }
            if (color[0], color[1], color[2]) == (rgb[0], rgb[1], rgb[2])
                && color[3] > 0.0
                && (*width - SCROLLBAR_INDICATOR_THICKNESS).abs() < 0.01 =>
        {
            Some(color[3])
        }
        _ => None,
    })
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

/// Make the active pointer modality Touch (a content flick is a Touch gesture)
/// and scroll the content through the Scroll Offset seam, then render at `now_ms`.
fn touch_scroll_at(tree: &mut ElementTree, scroll: ElementId, now_ms: f64) {
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    tree.on_pointer_up_with_kind(10.0, 10.0, PointerKind::Touch);
    tree.element_set_scroll_offset(scroll, 0.0, 40.0);
    tree.render(now_ms);
}

#[test]
fn touch_scroll_draws_no_operable_thumb() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    // The default modality is Mouse: the operable thumb is painted.
    assert_eq!(
        operable_thumbs(&tree).len(),
        1,
        "Mouse modality paints the operable thumb",
    );

    // Under Touch the operable Mouse/Pen bar must not be drawn — Touch gets the
    // transient indicator instead, which has no grabbable thumb (ADR-0110).
    touch_scroll_at(&mut tree, scroll, 0.0);
    assert!(
        operable_thumbs(&tree).is_empty(),
        "Touch modality draws no operable Mouse/Pen thumb",
    );
}

#[test]
fn touch_scroll_shows_transient_indicator() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    // A resting Touch surface paints no scrollbar — mobile has no always-on bar.
    // (The default modality is Mouse, so flip to Touch with a press first.)
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    tree.on_pointer_up_with_kind(10.0, 10.0, PointerKind::Touch);
    tree.render(0.0);
    assert!(
        vertical_indicators(&tree).is_empty(),
        "a Touch surface that is not scrolling shows no indicator",
    );

    // Scrolling the content raises the transient indicator: one vertical bar on
    // the overflowing axis, thinner than the operable thumb.
    touch_scroll_at(&mut tree, scroll, 0.0);
    let indicators = vertical_indicators(&tree);
    assert_eq!(
        indicators.len(),
        1,
        "a Touch scroll raises exactly one (vertical) transient indicator",
    );
    assert_eq!(
        indicators[0].2, SCROLLBAR_INDICATOR_THICKNESS,
        "the indicator is INDICATOR_THICKNESS wide",
    );
}

#[test]
fn touch_indicator_fades_out_after_scrolling_stops() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    touch_scroll_at(&mut tree, scroll, 0.0);
    assert_eq!(
        indicator_alpha(&tree),
        Some(SCROLLBAR_INDICATOR_OPACITY),
        "the indicator is fully visible while scrolling",
    );

    // Inside the hold window the indicator stays fully visible.
    tree.render(SCROLLBAR_INDICATOR_HOLD_MS / 2.0);
    assert_eq!(
        indicator_alpha(&tree),
        Some(SCROLLBAR_INDICATOR_OPACITY),
        "the indicator holds at full visibility before the fade begins",
    );

    // Partway through the fade window it is dimmer but still drawn.
    tree.render(SCROLLBAR_INDICATOR_HOLD_MS + SCROLLBAR_INDICATOR_FADE_MS / 2.0);
    let mid = indicator_alpha(&tree).expect("the indicator is still drawn mid-fade");
    assert!(
        mid > 0.0 && mid < SCROLLBAR_INDICATOR_OPACITY,
        "the indicator is fading (0 < {mid} < {SCROLLBAR_INDICATOR_OPACITY})",
    );

    // Past hold + fade it has faded out completely and is gone.
    tree.render(SCROLLBAR_INDICATOR_HOLD_MS + SCROLLBAR_INDICATOR_FADE_MS + 100.0);
    assert!(
        vertical_indicators(&tree).is_empty(),
        "the indicator fades out and disappears once scrolling stops",
    );
}

/// Under a precise pointer (Mouse/Pen) the overlay is the operable thumb (#409):
/// it is painted, carries no transient indicator, and a press + drag on it moves
/// the Scroll Offset. The S2 regression's precise-pointer arm (parametrized over
/// `PointerKind`, prior art: `selection_chrome_modality.rs`).
fn assert_precise_pointer_is_operable(kind: PointerKind) {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, kind);
    tree.on_pointer_up_with_kind(10.0, 10.0, kind);
    tree.render(0.0);

    let thumbs = operable_thumbs(&tree);
    assert_eq!(thumbs.len(), 1, "{kind:?} paints the operable thumb");
    assert!(
        vertical_indicators(&tree).is_empty(),
        "{kind:?} paints no transient indicator",
    );

    let (tx, ty, tw, th) = thumbs[0];
    let (cx, cy) = (tx + tw / 2.0, ty + th / 2.0);
    tree.on_pointer_down_with_kind(cx, cy, 0, kind);
    tree.on_pointer_move(cx, cy + 20.0);
    assert!(
        tree.element_get_scroll_offset(scroll).1 > 0.0,
        "a {kind:?} drag on the thumb operates the scrollbar",
    );
}

#[test]
fn mouse_and_pen_get_an_operable_thumb() {
    assert_precise_pointer_is_operable(PointerKind::Mouse);
    assert_precise_pointer_is_operable(PointerKind::Pen);
}

#[test]
fn touch_gets_a_non_operable_indicator() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    touch_scroll_at(&mut tree, scroll, 0.0);

    // Touch draws the transient indicator only — no operable Mouse/Pen thumb.
    let indicators = vertical_indicators(&tree);
    assert_eq!(indicators.len(), 1, "Touch draws the transient indicator");
    assert!(
        operable_thumbs(&tree).is_empty(),
        "Touch draws no operable thumb",
    );

    // The indicator has no thumb/track hit region: a press + drag on its pixels
    // does not operate a scrollbar (a real flick scrolls the content, not a bar).
    let (ix, iy, iw, ih) = indicators[0];
    let (cx, cy) = (ix + iw / 2.0, iy + ih / 2.0);
    let before = tree.element_get_scroll_offset(scroll).1;
    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Touch);
    tree.on_pointer_move(cx, cy + 30.0);
    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        before,
        "the Touch indicator has no hit region — the press operates no scrollbar",
    );
}
