//! Physical pointer device axis (#357, ADR-0104). `PointerKind` distinguishes
//! the device behind a pointer interaction — mouse, touch, or pen — and is
//! threaded from the Platform Adapter's `PointerEvent.pointerType` through
//! `on_pointer_down` / `on_pointer_move` / `on_pointer_up` into Core, which
//! retains the last kind per interaction (`last_pointer_kind`). It rides the
//! pointer proto/wire contract.
//!
//! Orthogonal to [`InputModality`](super::interaction::InputModality), the
//! Pointer/Keyboard axis driving `:focus-visible`: a touch press and a mouse
//! press are both `InputModality::Pointer` yet different `PointerKind`s. The two
//! axes coexist and are never conflated.

/// Which physical device produced a pointer interaction (#357). Mapped from the
/// DOM `PointerEvent.pointerType` at the Platform Adapter boundary and carried
/// on the pointer wire events so later slices (touch gates, I-beam modality)
/// can branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
}

impl PointerKind {
    /// Map a DOM `PointerEvent.pointerType` string. Unknown values (and the
    /// empty string some engines report) fall back to `Mouse` so they keep the
    /// mouse selection/drag path rather than hijacking the touch/pen gestures.
    pub fn from_dom(value: &str) -> Self {
        match value {
            "touch" => PointerKind::Touch,
            "pen" => PointerKind::Pen,
            _ => PointerKind::Mouse,
        }
    }

    /// Wire discriminant (`mouse=0`, `touch=1`, `pen=2`) for the pointer
    /// proto/wire contract. Paired with [`from_u32`](Self::from_u32).
    pub fn to_u32(self) -> u32 {
        match self {
            PointerKind::Mouse => 0,
            PointerKind::Touch => 1,
            PointerKind::Pen => 2,
        }
    }

    /// Inverse of [`to_u32`](Self::to_u32); an unknown discriminant falls back
    /// to `Mouse` (the same safe default `from_dom` uses for unknown types).
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => PointerKind::Touch,
            2 => PointerKind::Pen,
            _ => PointerKind::Mouse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_dom_pointer_type_strings_and_defaults_to_mouse() {
        assert_eq!(PointerKind::from_dom("mouse"), PointerKind::Mouse);
        assert_eq!(PointerKind::from_dom("touch"), PointerKind::Touch);
        assert_eq!(PointerKind::from_dom("pen"), PointerKind::Pen);
        // Unknown / empty pointerType keeps the mouse path.
        assert_eq!(PointerKind::from_dom(""), PointerKind::Mouse);
        assert_eq!(PointerKind::from_dom("eraser"), PointerKind::Mouse);
    }

    #[test]
    fn wire_discriminant_round_trips() {
        for kind in [PointerKind::Mouse, PointerKind::Touch, PointerKind::Pen] {
            assert_eq!(PointerKind::from_u32(kind.to_u32()), kind);
        }
        // Pinned wire values — the proto/wire contract must stay stable.
        assert_eq!(PointerKind::Mouse.to_u32(), 0);
        assert_eq!(PointerKind::Touch.to_u32(), 1);
        assert_eq!(PointerKind::Pen.to_u32(), 2);
        // An out-of-range discriminant decodes to the safe default.
        assert_eq!(PointerKind::from_u32(99), PointerKind::Mouse);
    }
}
