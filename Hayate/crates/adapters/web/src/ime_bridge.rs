use hayate_core::{CharacterBounds, ElementId, ElementTree, ImeBridge};

/// Web EditContext bridge: stores last character bounds for JS to apply via
/// `EditContext.updateControlBounds` / `updateSelectionBounds` (ADR-0069).
#[derive(Clone, Copy, Debug)]
pub struct WebImeBridge {
    last_bounds: CharacterBounds,
}

impl Default for WebImeBridge {
    fn default() -> Self {
        Self {
            last_bounds: CharacterBounds {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
        }
    }
}

impl WebImeBridge {
    pub fn last_bounds(&self) -> CharacterBounds {
        self.last_bounds
    }
}

impl ImeBridge for WebImeBridge {
    fn update_character_bounds(&mut self, bounds: CharacterBounds) {
        self.last_bounds = bounds;
    }
}

/// Push cursor character bounds from core into the platform IME bridge after layout.
pub fn sync_ime_character_bounds(
    tree: &ElementTree,
    focused: ElementId,
    ime: &mut impl ImeBridge,
) {
    if let Some(bounds) = tree.element_character_bounds(focused) {
        ime.update_character_bounds(bounds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{Dimension, ElementKind, StyleProp};

    #[test]
    fn sync_updates_bridge_after_layout() {
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
        sync_ime_character_bounds(&tree, input, &mut bridge);
        let bounds = bridge.last_bounds();
        assert!(bounds.width > 0.0);
        assert!(bounds.height > 0.0);
    }
}
