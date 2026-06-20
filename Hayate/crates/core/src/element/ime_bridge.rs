/// Screen-space character bounds for IME candidate window placement (ADR-0069).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// What the platform IME should surface this frame, computed once by core
/// ([`ElementTree::drive_ime`](crate::ElementTree::drive_ime)) from editability
/// (#392). Adapters reflect this and nothing more — soft-keyboard visibility is
/// no longer re-derived per platform, so a gating fix lands once for everyone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImePresentation {
    /// No editable element is focused. The adapter dismisses the soft keyboard.
    /// A tap focuses whatever it hits (buttons, plain text, views — Chromium
    /// parity, ADR-0102), but only a `text-input` is editable, so a plain tap
    /// lands here and must not raise the keyboard (#392).
    Hidden,
    /// A `text-input` is focused. The adapter shows the soft keyboard and points
    /// the IME candidate window at `bounds`.
    Shown { bounds: CharacterBounds },
}

/// Platform IME plumbing seam (ADR-0069). Adapters wrap EditContext (web) /
/// GameTextInput (Android) / TSF / TSM / IBus and do nothing beyond reflecting
/// [`ImePresentation`]; the editability decision — *whether* the keyboard shows
/// and *where* the candidate window sits — lives in core. Keeping that decision
/// out of the adapters is what prevents a per-platform divergence like #392
/// (fixed for Android only because each adapter hand-rolled the gate).
pub trait ImeBridge {
    fn present(&mut self, presentation: ImePresentation);
}
