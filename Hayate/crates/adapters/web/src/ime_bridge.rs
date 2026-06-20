use hayate_core::{CharacterBounds, ImeBridge, ImePresentation};

/// Web EditContext bridge (ADR-0069, #392). It only *reflects* the
/// [`ImePresentation`] core computes each frame: whether a `text-input` is
/// focused (`visible`) and where its caret sits (`last_bounds`). The JS host
/// reads `visible` to attach/detach `EditContext` — which is what shows or
/// dismisses the mobile soft keyboard — and `last_bounds` to place the candidate
/// window via `updateControlBounds` / `updateSelectionBounds`. The adapter makes
/// no editability decision of its own; that lives in `ElementTree::drive_ime`.
#[derive(Clone, Copy, Debug, Default)]
pub struct WebImeBridge {
    last_bounds: CharacterBounds,
    visible: bool,
}

impl WebImeBridge {
    pub fn last_bounds(&self) -> CharacterBounds {
        self.last_bounds
    }

    /// Whether core wants the soft keyboard up this frame (a `text-input` is
    /// focused). The JS host attaches `EditContext` only while this is true.
    pub fn visible(&self) -> bool {
        self.visible
    }
}

impl ImeBridge for WebImeBridge {
    fn present(&mut self, presentation: ImePresentation) {
        match presentation {
            ImePresentation::Hidden => self.visible = false,
            ImePresentation::Shown { bounds } => {
                self.visible = true;
                self.last_bounds = bounds;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{Dimension, ElementKind, ElementTree, StyleProp};

    #[test]
    fn focused_text_input_makes_the_bridge_visible_with_bounds() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        tree.set_viewport(200.0, 40.0);
        tree.element_set_style(
            input,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(40.0)),
                StyleProp::FontSize(16.0),
            ],
        );
        tree.element_append_text_content(input, "hi");
        tree.render(0.0);

        let mut bridge = WebImeBridge::default();
        tree.drive_ime(&mut bridge);
        assert!(bridge.visible(), "text-input focus must arm the keyboard");
        let bounds = bridge.last_bounds();
        assert!(bounds.width > 0.0);
        assert!(bounds.height > 0.0);
    }

    #[test]
    fn focusing_a_non_input_keeps_the_bridge_hidden() {
        let mut tree = ElementTree::new();
        let view = tree.element_create(1, ElementKind::View);
        let text = tree.element_create(2, ElementKind::Text);
        tree.element_append_child(view, text);
        tree.set_root(view);
        tree.set_viewport(200.0, 40.0);
        tree.element_focus(text);
        tree.render(0.0);

        let mut bridge = WebImeBridge::default();
        tree.drive_ime(&mut bridge);
        assert!(
            !bridge.visible(),
            "focusing plain text must not arm the soft keyboard (#392)"
        );
    }
}
