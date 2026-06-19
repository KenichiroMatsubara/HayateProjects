//! A scroll-view is a CSS scroll container, so as a flex item its automatic
//! minimum size is 0 and it shrinks to the space its siblings leave — it does
//! not overflow the parent by a fixed-height sibling's extent.
//!
//! Regression: in Tsubame Task Studio (both the Tasks and CSS Gallery pages) a
//! `height: 100%` scroll-view sits below a fixed-height AppBar in a flex column.
//! Before the fix, Canvas mode laid the scroll-view out at the full window
//! height, overflowing the window bottom by the AppBar height. That inflated box
//! height is also the scroll viewport (`element_scroll_max_offset`), so the last
//! AppBar-height band of content was unreachable — scrolling stopped short of
//! the bottom while DOM mode (native scroll) reached it. Marking ScrollView as a
//! Taffy scroll container restores the browser's shrink-to-fit (Semantics
//! Parity). Covers both the Tasks variant (`flex-grow: 1`) and the Gallery
//! variant (`height: 100%` only).

use hayate_core::{Dimension, ElementKind, ElementTree, FlexDirectionValue, StyleProp};

const WINDOW_H: f32 = 800.0;
const APPBAR_H: f32 = 64.0;
const CONTENT_H: f32 = 2000.0;
const PAD_BOTTOM: f32 = 28.0;

fn build(tasks_variant: bool) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let appbar = tree.element_create(2, ElementKind::View);
    let scroll = tree.element_create(3, ElementKind::ScrollView);
    let content = tree.element_create(4, ElementKind::View);

    tree.set_root(root);
    tree.set_viewport(1200.0, WINDOW_H);

    // root: full-window flex column
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    // appbar: fixed 64px tall. In the real app it holds 64px via its content
    // (logo, buttons) — model that with flex-shrink:0 so it can't collapse.
    tree.element_set_style(
        appbar,
        &[
            StyleProp::Height(Dimension::px(APPBAR_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    // scroll-view: the App.tsx / CssGallery.tsx pattern.
    let mut sv_style = vec![
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::percent(100.0)),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::PaddingBottom(Dimension::px(PAD_BOTTOM)),
    ];
    if tasks_variant {
        sv_style.push(StyleProp::FlexGrow(1.0));
    }
    tree.element_set_style(scroll, &sv_style);
    // tall content child. In the real app this column's height comes from its
    // many children, so min-content (min-height:auto) keeps it from shrinking
    // below content. Model that with flex-shrink:0.
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::px(CONTENT_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );

    tree.element_append_child(root, appbar);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

fn check(tasks_variant: bool) {
    let label = if tasks_variant { "Tasks" } else { "Gallery" };
    let (tree, scroll) = build(tasks_variant);

    let (_, sv_top, _, view_h) = tree.element_layout_rect(scroll).unwrap();
    let (_max_x, max_y) = tree.element_scroll_max_offset(scroll);

    // The scroll-view starts under the AppBar and fills only the leftover height.
    let expected_viewport = WINDOW_H - APPBAR_H;
    // Scrollable content = the content child plus the scroll-view's bottom padding.
    let expected_max = (CONTENT_H + PAD_BOTTOM) - expected_viewport;

    assert!(
        (sv_top - APPBAR_H).abs() < 0.5,
        "{label}: scroll-view top {sv_top}, expected {APPBAR_H} (below the AppBar)"
    );
    assert!(
        (view_h - expected_viewport).abs() < 0.5,
        "{label}: scroll-view viewport {view_h}, expected {expected_viewport} \
         (window {WINDOW_H} - appbar {APPBAR_H}); it must shrink, not overflow"
    );
    assert!(
        (max_y - expected_max).abs() < 0.5,
        "{label}: max scroll {max_y}, expected {expected_max} — short by {} (content unreachable)",
        expected_max - max_y
    );
}

#[test]
fn tasks_page_scroll_view_shrinks_and_reaches_bottom() {
    check(true);
}

#[test]
fn gallery_page_scroll_view_shrinks_and_reaches_bottom() {
    check(false);
}
