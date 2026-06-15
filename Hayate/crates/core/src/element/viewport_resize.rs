//! Resize → `(shape, visual)` dirty resolution in one place (ADR-0081, #324).
//!
//! "Did an element's viewport-conditioned style change on resize, and if so how
//! must it be marked?" used to be spread across three seams: the element scan in
//! `tree.rs`, the old/new comparison in `effective_visual.rs`, and the
//! shape-vs-visual classification in `engine.rs`'s viewport promotion. This
//! module folds all three into a single pure function so the caller only raises
//! dirty from the returned sets — no intermediate `viewport_dirty` set, no
//! implicit scan → promote → compare ordering.

use std::collections::HashSet;

use crate::element::effective_visual::own_with_viewport_variants;
use crate::element::id::ElementId;
use crate::element::style::{StyleProp, ViewportCondition};
use crate::element::tree::{apply_visual, Visual};

/// One element's resize-relevant state: its base visual and viewport variants.
pub(crate) struct ElementResizeInput<'a> {
    pub id: ElementId,
    pub base: &'a Visual,
    pub variants: &'a [(ViewportCondition, StyleProp)],
}

/// Which elements changed under a resize, split by how they must be marked.
#[derive(Default, Debug)]
pub(crate) struct ViewportResizeDirty {
    /// Variant touches a text field → re-shape (Parley) + projection dirty.
    pub shape: HashSet<ElementId>,
    /// Variant is box-visual only → scene re-lower, no re-shape.
    pub visual: HashSet<ElementId>,
}

/// Resolve which elements need marking after a viewport resize.
///
/// For each element carrying viewport variants, compares the variant-resolved
/// own-style at the old vs new viewport (responsibility: *which* elements
/// changed), then classifies the change as shape or visual by whether the
/// variants active at the new viewport touch a text field (responsibility:
/// *how* to mark). Elements without variants, or whose resolution is unchanged,
/// are skipped.
pub(crate) fn resolve_resize<'a>(
    elements: impl IntoIterator<Item = ElementResizeInput<'a>>,
    old_viewport: (f32, f32),
    new_viewport: (f32, f32),
) -> ViewportResizeDirty {
    let mut dirty = ViewportResizeDirty::default();
    if old_viewport == new_viewport {
        return dirty;
    }
    for el in elements {
        if el.variants.is_empty() {
            continue;
        }
        if !resolution_changed(el.base, el.variants, old_viewport, new_viewport) {
            continue;
        }
        if variants_touch_text_at(el.variants, new_viewport) {
            dirty.shape.insert(el.id);
        } else {
            dirty.visual.insert(el.id);
        }
    }
    dirty
}

/// Whether the viewport-conditioned own-style differs between two viewports.
fn resolution_changed(
    base: &Visual,
    variants: &[(ViewportCondition, StyleProp)],
    old_viewport: (f32, f32),
    new_viewport: (f32, f32),
) -> bool {
    let old = own_with_viewport_variants(base, variants, old_viewport);
    let new = own_with_viewport_variants(base, variants, new_viewport);
    own_visual_differs(&old, &new)
}

/// Whether any variant active at `viewport` sets a text field (font/color/etc.),
/// which forces a Parley re-shape rather than a paint-only re-lower.
fn variants_touch_text_at(
    variants: &[(ViewportCondition, StyleProp)],
    viewport: (f32, f32),
) -> bool {
    let mut probe = Visual::default();
    let mut text_dirty = false;
    for (condition, prop) in variants {
        if condition.matches(viewport.0, viewport.1) {
            apply_visual(&mut probe, prop, &mut text_dirty);
        }
    }
    text_dirty
}

fn own_visual_differs(a: &Visual, b: &Visual) -> bool {
    a.background_color != b.background_color
        || (a.opacity - b.opacity).abs() > f32::EPSILON
        || (a.border_radius - b.border_radius).abs() > f32::EPSILON
        || (a.border_width - b.border_width).abs() > f32::EPSILON
        || a.border_color != b.border_color
        || a.border_style != b.border_style
        || a.box_shadow != b.box_shadow
        || a.overflow != b.overflow
        || a.max_lines != b.max_lines
        || a.text_overflow != b.text_overflow
        || a.text_color != b.text_color
        || a.font_size != b.font_size
        || a.font_weight != b.font_weight
        || a.font_style != b.font_style
        || a.text_decoration != b.text_decoration
        || a.z_index != b.z_index
        || a.font_family != b.font_family
        || a.default_color != b.default_color
        || a.default_font_size != b.default_font_size
        || a.default_font_weight != b.default_font_weight
        || a.default_font_family != b.default_font_family
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn variant(min_width: f32, prop: StyleProp) -> (ViewportCondition, StyleProp) {
        (
            ViewportCondition {
                min_width: Some(min_width),
                ..Default::default()
            },
            prop,
        )
    }

    #[test]
    fn background_variant_crossing_breakpoint_is_a_visual_change() {
        let base = Visual::default();
        let variants = vec![variant(
            768.0,
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        )];
        let id = ElementId::from_u64(1);

        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert!(dirty.visual.contains(&id), "background variant marks visual");
        assert!(!dirty.shape.contains(&id), "background variant is not shape");
    }

    #[test]
    fn text_variant_crossing_breakpoint_is_a_shape_change() {
        let base = Visual::default();
        let variants = vec![variant(768.0, StyleProp::FontSize(24.0))];
        let id = ElementId::from_u64(2);

        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert!(dirty.shape.contains(&id), "font-size variant marks shape");
        assert!(!dirty.visual.contains(&id), "font-size variant is not visual");
    }

    #[test]
    fn element_without_variants_is_skipped() {
        let base = Visual::default();
        let variants: Vec<(ViewportCondition, StyleProp)> = Vec::new();
        let id = ElementId::from_u64(3);

        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert!(dirty.shape.is_empty() && dirty.visual.is_empty());
    }

    #[test]
    fn resize_within_same_breakpoint_changes_nothing() {
        let base = Visual::default();
        let variants = vec![variant(
            768.0,
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        )];
        let id = ElementId::from_u64(4);

        // Both viewports are above the 768px breakpoint: resolution is unchanged.
        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (900.0, 800.0),
            (950.0, 850.0),
        );

        assert!(dirty.shape.is_empty() && dirty.visual.is_empty());
    }

    #[test]
    fn mixed_elements_partition_into_shape_and_visual() {
        let base = Visual::default();
        let bg = vec![variant(
            768.0,
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        )];
        let font = vec![variant(768.0, StyleProp::FontSize(24.0))];
        let none: Vec<(ViewportCondition, StyleProp)> = Vec::new();
        let (visual_id, shape_id, clean_id) = (
            ElementId::from_u64(10),
            ElementId::from_u64(11),
            ElementId::from_u64(12),
        );

        let dirty = resolve_resize(
            [
                ElementResizeInput {
                    id: visual_id,
                    base: &base,
                    variants: &bg,
                },
                ElementResizeInput {
                    id: shape_id,
                    base: &base,
                    variants: &font,
                },
                ElementResizeInput {
                    id: clean_id,
                    base: &base,
                    variants: &none,
                },
            ],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert_eq!(dirty.visual, HashSet::from([visual_id]));
        assert_eq!(dirty.shape, HashSet::from([shape_id]));
    }
}
