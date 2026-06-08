use hayate_core::{CharacterBounds, ImeBridge};

/// Web EditContext bridge: stores last character bounds for JS to apply via
/// `EditContext.updateControlBounds` / `updateSelectionBounds` (ADR-0069).
#[derive(Clone, Copy, Debug, Default)]
pub struct WebImeBridge {
    last_bounds: CharacterBounds,
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
