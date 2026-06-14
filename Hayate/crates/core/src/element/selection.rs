//! Unified text-selection model (ADR-0097).
//!
//! A single `Selection` is owned by the Element Document Runtime
//! (`ElementTree`); at most one is active across the whole document. Both
//! endpoints are `(ElementId, byte offset)` pairs. A degenerate selection with
//! `anchor == focus` is a caret — the growth point that subsumes
//! `EditState::cursor_byte_index` (ADR-0069).

use crate::element::id::ElementId;

/// One end of a selection: a byte offset within a specific element's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionPoint {
    pub element: ElementId,
    pub offset: usize,
}

impl SelectionPoint {
    pub fn new(element: ElementId, offset: usize) -> Self {
        Self { element, offset }
    }
}

/// A continuous text selection between `anchor` (where the drag started) and
/// `focus` (where the pointer currently is). Document order is not implied by
/// field order; callers normalize via [`Selection::range_within`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: SelectionPoint,
    pub focus: SelectionPoint,
}

impl Selection {
    /// A degenerate (collapsed) selection at a single point — a caret.
    pub fn caret(point: SelectionPoint) -> Self {
        Self {
            anchor: point,
            focus: point,
        }
    }

    /// True when the selection is collapsed (anchor == focus): a caret.
    pub fn is_caret(&self) -> bool {
        self.anchor == self.focus
    }

    /// The selected byte range within `element`, normalized to document order
    /// (`start <= end`). Returns `None` unless both endpoints lie in `element`
    /// (the single-IFC tracer case; cross-element selection is a growth point).
    pub fn range_within(&self, element: ElementId) -> Option<(usize, usize)> {
        if self.anchor.element != element || self.focus.element != element {
            return None;
        }
        let a = self.anchor.offset;
        let b = self.focus.offset;
        Some((a.min(b), a.max(b)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(id: u64, offset: usize) -> SelectionPoint {
        SelectionPoint::new(ElementId::from_u64(id), offset)
    }

    #[test]
    fn caret_is_collapsed_at_a_single_point() {
        let sel = Selection::caret(point(1, 3));
        assert!(sel.is_caret());
        assert_eq!(sel.anchor, sel.focus);
        assert_eq!(sel.range_within(ElementId::from_u64(1)), Some((3, 3)));
    }

    #[test]
    fn range_within_normalizes_to_document_order() {
        // focus before anchor (drag leftwards) still yields start <= end.
        let sel = Selection {
            anchor: point(1, 7),
            focus: point(1, 2),
        };
        assert!(!sel.is_caret());
        assert_eq!(sel.range_within(ElementId::from_u64(1)), Some((2, 7)));
    }

    #[test]
    fn range_within_is_none_for_a_different_element() {
        let sel = Selection {
            anchor: point(1, 0),
            focus: point(1, 4),
        };
        assert_eq!(sel.range_within(ElementId::from_u64(2)), None);
    }
}
