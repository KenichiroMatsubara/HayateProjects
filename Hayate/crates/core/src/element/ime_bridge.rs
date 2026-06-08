/// Screen-space character bounds for IME candidate window placement (ADR-0069).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Platform IME plumbing seam. Adapters wrap EditContext / TSF / IBus only.
pub trait ImeBridge {
    fn update_character_bounds(&mut self, bounds: CharacterBounds);
}
