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
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, FlexDirectionValue,
    JustifyValue, PositionValue, PseudoState, StyleProp,
};

/// Stable element ids for the demo tree (so on-device logs can refer to them,
/// mirroring how `hayate-adapter-web` assigns ids from the JS side).
pub const ROOT_ID: u64 = 1;
pub const BUTTON_ID: u64 = 2;
pub const TEXT_INPUT_ID: u64 = 3;
/// A `selectable` paragraph (its Text child is the IFC) demonstrating the
/// read-only SelectionArea floating toolbar (ADR-0097, #272).
pub const PARAGRAPH_ID: u64 = 4;
pub const PARAGRAPH_TEXT_ID: u64 = 5;

/// The selectable demo paragraph's copy.
pub const PARAGRAPH_TEXT: &str = "Drag to select this text";

/// Idle (un-pressed) button background.
pub const BUTTON_IDLE: Color = Color::new(0.16, 0.45, 0.92, 1.0);
/// `:active` (pressed) button background — flips visibly under a finger.
pub const BUTTON_ACTIVE: Color = Color::new(0.92, 0.35, 0.16, 1.0);

/// Build the demo tree: a full-viewport flex column centering a button that
/// flips color while pressed (stage B) above a text-input that receives the
/// soft keyboard for the stage C IME bridge (ADR-0094).
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
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::Gap(Dimension::px(24.0)),
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

    let input = tree.element_create(TEXT_INPUT_ID, ElementKind::TextInput);
    tree.element_append_child(root, input);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(260.0)),
            StyleProp::Height(Dimension::px(56.0)),
            StyleProp::BorderRadius(8.0),
            StyleProp::BorderWidth(2.0),
            StyleProp::BorderColor(Color::new(0.4, 0.4, 0.45, 1.0)),
            StyleProp::BackgroundColor(Color::WHITE),
            StyleProp::FontSize(20.0),
            StyleProp::Color(Color::BLACK),
        ],
    );

    // A selectable paragraph pinned to the top of the viewport (absolute, so it
    // does not disturb the centered button/input column). Dragging across it
    // raises the core-drawn Material selection toolbar with Copy / Select All
    // (ADR-0097, #272).
    let paragraph = tree.element_create(PARAGRAPH_ID, ElementKind::View);
    tree.element_append_child(root, paragraph);
    tree.element_set_style(
        paragraph,
        &[
            StyleProp::Position(PositionValue::Absolute),
            StyleProp::Top(Dimension::px(24.0)),
            StyleProp::Left(Dimension::px(24.0)),
            StyleProp::Width(Dimension::px(320.0)),
        ],
    );
    tree.element_set_selectable(paragraph, true);

    let paragraph_text = tree.element_create(PARAGRAPH_TEXT_ID, ElementKind::Text);
    tree.element_append_child(paragraph, paragraph_text);
    tree.element_set_style(
        paragraph_text,
        &[
            StyleProp::Width(Dimension::px(320.0)),
            StyleProp::FontSize(20.0),
            StyleProp::Color(Color::new(0.1, 0.1, 0.12, 1.0)),
        ],
    );
    tree.element_set_text(paragraph_text, PARAGRAPH_TEXT);

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

    // Pressing the button must flip its effective background to the `:active`
    // color and release must restore it — the end-to-end behavior the on-device
    // tap is meant to make visible.
    #[test]
    fn pressing_the_button_flips_its_background() {
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        let button = ElementId::from_u64(BUTTON_ID);

        // Column-centered in 400×800: button spans x 90..310, y 312..408; its
        // center is (200, 360).
        tree.on_pointer_down(200.0, 360.0);
        assert_eq!(
            tree.element_effective_visual(button)
                .expect("button visual")
                .background_color,
            Some(BUTTON_ACTIVE),
            "press should flip to the :active background"
        );

        tree.on_pointer_up(200.0, 360.0);
        assert_eq!(
            tree.element_effective_visual(button)
                .expect("button visual")
                .background_color,
            Some(BUTTON_IDLE),
            "release should restore the idle background"
        );
    }

    // Tapping the text-input focuses it, which is the precondition for the glue
    // to show the soft keyboard and route GameTextInput into it (stage C).
    #[test]
    fn tapping_the_text_input_focuses_it() {
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        // The text-input sits below the button at y 432..488; center (200, 460).
        tree.on_pointer_down(200.0, 460.0);
        tree.on_pointer_up(200.0, 460.0);

        assert_eq!(
            tree.focused_element(),
            Some(ElementId::from_u64(TEXT_INPUT_ID)),
            "tapping the text-input should focus it"
        );
    }

    // Dragging across the selectable paragraph raises the core-drawn floating
    // toolbar offering Copy / Select All — the read-only SelectionArea chrome the
    // on-device check makes visible (ADR-0097, #272).
    #[test]
    fn dragging_the_paragraph_shows_the_selection_toolbar() {
        use hayate_core::ToolbarAction;
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        // The paragraph sits at absolute (24, 24); drag across its first glyphs.
        tree.on_pointer_down(28.0, 32.0);
        tree.on_pointer_move(120.0, 32.0);
        tree.on_pointer_up(120.0, 32.0);

        let toolbar = tree
            .selection_toolbar()
            .expect("the selection toolbar is shown after dragging the paragraph");
        assert_eq!(
            toolbar.actions(),
            vec![ToolbarAction::Copy, ToolbarAction::SelectAll],
        );
    }
}
