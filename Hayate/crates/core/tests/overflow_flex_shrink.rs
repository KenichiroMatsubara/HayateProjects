//! The `overflow` prop reaches Taffy, not just the visual clip. A non-`visible`
//! overflow makes a box a CSS scroll container, whose flex automatic minimum
//! size is 0, so it shrinks to the space its siblings leave instead of
//! overflowing them. This is the general-prop counterpart of the scroll-view
//! kind default (see `scroll_view_flex_shrink.rs`): before the fix the bridge
//! dropped `overflow` entirely, so even `overflow: hidden` boxes refused to
//! shrink, diverging from the browser (DOM mode).

use hayate_core::{
    Dimension, ElementKind, ElementTree, FlexDirectionValue, OverflowValue, StyleProp,
};

const WINDOW_H: f32 = 800.0;
const BAR_H: f32 = 64.0;
const CONTENT_H: f32 = 2000.0;
const LEFTOVER: f32 = WINDOW_H - BAR_H; // 736: the space the box should shrink to

/// A flex column: a fixed-height bar (can't shrink) above a `view` sized
/// `height: 100%` whose own child is taller than the window. Returns the tree
/// and the middle box so a test can toggle its `overflow` and re-measure.
fn build(overflow: Option<OverflowValue>) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let bar = tree.element_create(2, ElementKind::View);
    let box_ = tree.element_create(3, ElementKind::View);
    let content = tree.element_create(4, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(1200.0, WINDOW_H);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        bar,
        &[
            StyleProp::Height(Dimension::px(BAR_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    let mut s = vec![
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::percent(100.0)),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
    ];
    if let Some(o) = overflow {
        s.push(StyleProp::Overflow(o));
    }
    tree.element_set_style(box_, &s);
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::px(CONTENT_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_append_child(root, bar);
    tree.element_append_child(root, box_);
    tree.element_append_child(box_, content);
    tree.render(0.0);
    (tree, box_)
}

fn box_height(tree: &ElementTree, box_: hayate_core::ElementId) -> f32 {
    tree.element_layout_rect(box_).unwrap().3
}

#[test]
fn overflow_hidden_view_shrinks_as_flex_item() {
    let (visible, vbox) = build(Some(OverflowValue::Visible));
    assert!(
        (box_height(&visible, vbox) - WINDOW_H).abs() < 0.5,
        "overflow:visible should not shrink (stays the full basis, overflowing): got {}",
        box_height(&visible, vbox)
    );

    let (hidden, hbox) = build(Some(OverflowValue::Hidden));
    assert!(
        (box_height(&hidden, hbox) - LEFTOVER).abs() < 0.5,
        "overflow:hidden is a scroll container and must shrink to {LEFTOVER}: got {}",
        box_height(&hidden, hbox)
    );
}

#[test]
fn toggling_overflow_at_runtime_re_runs_layout() {
    // Start visible: the box keeps its full basis and overflows the window.
    let (mut tree, box_) = build(Some(OverflowValue::Visible));
    assert!((box_height(&tree, box_) - WINDOW_H).abs() < 0.5);

    // Flip to hidden: the dual-routing must mark the Taffy node layout-dirty,
    // so the next render shrinks it to the leftover space.
    tree.element_set_style(box_, &[StyleProp::Overflow(OverflowValue::Hidden)]);
    tree.render(0.0);
    assert!(
        (box_height(&tree, box_) - LEFTOVER).abs() < 0.5,
        "toggling to overflow:hidden must re-run layout and shrink to {LEFTOVER}: got {}",
        box_height(&tree, box_)
    );

    // Flip back to visible: it grows back to the full basis.
    tree.element_set_style(box_, &[StyleProp::Overflow(OverflowValue::Visible)]);
    tree.render(0.0);
    assert!(
        (box_height(&tree, box_) - WINDOW_H).abs() < 0.5,
        "toggling back to overflow:visible must re-run layout and restore {WINDOW_H}: got {}",
        box_height(&tree, box_)
    );
}
