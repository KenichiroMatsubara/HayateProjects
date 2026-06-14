//! Minimal interactive element tree for the stage B on-device check (ADR-0087).
//!
//! Stage A rendered an empty `SceneGraph` — only the clear color reached the
//! screen, so the `ElementTree -> SceneGraph -> Vello` pipe and the touch
//! wiring (`translate_touch`) had nothing observable to confirm. Stage B builds
//! a single button centered in the viewport whose `:active` background flips
//! while pressed, so a tap visibly changes pixels end-to-end.
//!
//! The builder uses only `hayate-core` element APIs (no NDK), so it is the
//! host-testable seam for "the demo tree is interactive"; `app.rs` is the thin
//! glue that sizes the viewport and renders it each frame.

use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, JustifyValue,
    PseudoState, StyleProp,
};

/// Stable element ids for the stage B demo tree (so on-device logs can refer to
/// them, mirroring how `hayate-adapter-web` assigns ids from the JS side).
pub const ROOT_ID: u64 = 1;
pub const BUTTON_ID: u64 = 2;

/// Idle (un-pressed) button background.
pub const BUTTON_IDLE: Color = Color::new(0.16, 0.45, 0.92, 1.0);
/// `:active` (pressed) button background — flips visibly under a finger.
pub const BUTTON_ACTIVE: Color = Color::new(0.92, 0.35, 0.16, 1.0);

/// Build the stage B demo tree: a full-viewport flex container centering a
/// button that flips its background color while pressed.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn build_demo_tree() -> ElementTree {
    let mut tree = ElementTree::new();

    let root = tree.element_create(ROOT_ID, ElementKind::View);
    tree.set_root(root);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::AlignItems(AlignValue::Center),
        ],
    );

    let button = tree.element_create(BUTTON_ID, ElementKind::Button);
    tree.element_append_child(root, button);
    tree.element_set_style(
        button,
        &[
            StyleProp::Width(Dimension::px(220.0)),
            StyleProp::Height(Dimension::px(96.0)),
            StyleProp::BorderRadius(16.0),
            StyleProp::BackgroundColor(BUTTON_IDLE),
        ],
    );
    tree.element_set_pseudo_style(
        button,
        PseudoState::Active,
        &[StyleProp::BackgroundColor(BUTTON_ACTIVE)],
    );

    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::ElementId;

    #[test]
    fn demo_button_starts_at_the_idle_color() {
        let tree = build_demo_tree();
        let button = ElementId::from_u64(BUTTON_ID);
        let visual = tree
            .element_effective_visual(button)
            .expect("button has an effective visual");
        assert_eq!(visual.background_color, Some(BUTTON_IDLE));
    }

    // Pressing the centered button must flip its effective background to the
    // `:active` color and release must restore it — this is the end-to-end
    // behavior the on-device tap is meant to make visible.
    #[test]
    fn pressing_the_centered_button_flips_its_background() {
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        let button = ElementId::from_u64(BUTTON_ID);

        // 220×96 button centered in a 400×800 viewport contains (200, 400).
        tree.on_pointer_down(200.0, 400.0);
        assert_eq!(
            tree.element_effective_visual(button)
                .expect("button visual")
                .background_color,
            Some(BUTTON_ACTIVE),
            "press should flip to the :active background"
        );

        tree.on_pointer_up(200.0, 400.0);
        assert_eq!(
            tree.element_effective_visual(button)
                .expect("button visual")
                .background_color,
            Some(BUTTON_IDLE),
            "release should restore the idle background"
        );
    }
}
