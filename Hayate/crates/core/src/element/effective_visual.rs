use std::collections::HashMap;

use crate::element::ambient_defaults::{self, AmbientDefaults};
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::pseudo_state::{self, InteractionSnapshot, PseudoStyles};
use crate::element::style::{FontStyleValue, StyleProp, TextDecorationValue, ViewportCondition};
use crate::element::tree::{apply_visual, Element, Visual};
use crate::color::Color;

/// ch1 text→text inherited text-style fields (ADR-0065).
#[derive(Clone, Debug)]
pub struct TextLocalInherited {
    pub color: Color,
    pub font_size: f32,
    pub font_weight: Option<f32>,
    pub font_family: Option<String>,
    pub font_style: Option<FontStyleValue>,
    pub text_decoration: Option<TextDecorationValue>,
}

impl TextLocalInherited {
    pub fn from_inherited_base(visual: &Visual) -> Self {
        Self {
            color: visual.text_color.unwrap_or(Color::BLACK),
            font_size: visual.font_size.unwrap_or(16.0),
            font_weight: visual.font_weight,
            font_family: visual.font_family.clone(),
            font_style: visual.font_style,
            text_decoration: visual.text_decoration,
        }
    }
}

/// Inherited context for effective visual resolution (ADR-0067).
#[derive(Clone, Debug)]
pub struct InheritedVisualContext {
    pub ambient: AmbientDefaults,
    pub text_local: Option<TextLocalInherited>,
}

impl InheritedVisualContext {
    pub fn root() -> Self {
        Self {
            ambient: AmbientDefaults::hard(),
            text_local: None,
        }
    }
}

/// Apply ch1 + ch2 inheritance onto unset text/visual fields of `own`.
pub fn apply_text_inheritance(ctx: &InheritedVisualContext, own: &Visual) -> Visual {
    let mut v = own.clone();
    if v.text_color.is_none() {
        v.text_color = ctx
            .text_local
            .as_ref()
            .map(|t| t.color)
            .or(Some(ctx.ambient.color));
    }
    if v.font_size.is_none() {
        v.font_size = Some(
            ctx.text_local
                .as_ref()
                .map(|t| t.font_size)
                .unwrap_or(ctx.ambient.font_size),
        );
    }
    if v.font_weight.is_none() {
        v.font_weight = ctx
            .text_local
            .as_ref()
            .and_then(|t| t.font_weight)
            .or(ctx.ambient.font_weight);
    }
    if v.font_family.is_none() {
        v.font_family = ctx
            .text_local
            .as_ref()
            .and_then(|t| t.font_family.clone())
            .or_else(|| ctx.ambient.font_family.clone());
    }
    if v.font_style.is_none() {
        v.font_style = ctx.text_local.as_ref().and_then(|t| t.font_style);
    }
    if v.text_decoration.is_none() {
        v.text_decoration = ctx.text_local.as_ref().and_then(|t| t.text_decoration);
    }
    v
}

/// Apply matching viewport variants onto a copy of `base` (ADR-0081).
pub fn own_with_viewport_variants(
    base: &Visual,
    variants: &[(ViewportCondition, StyleProp)],
    viewport: (f32, f32),
) -> Visual {
    let mut own = base.clone();
    let (viewport_width, viewport_height) = viewport;
    let mut text_dirty = false;
    for (condition, prop) in variants {
        if condition.matches(viewport_width, viewport_height) {
            apply_visual(&mut own, prop, &mut text_dirty);
        }
    }
    let _ = text_dirty;
    own
}

/// Whether viewport-conditioned own-style would change between two viewport sizes.
pub fn viewport_variant_resolution_changed(
    base: &Visual,
    variants: &[(ViewportCondition, StyleProp)],
    old_viewport: (f32, f32),
    new_viewport: (f32, f32),
) -> bool {
    if variants.is_empty() || old_viewport == new_viewport {
        return false;
    }
    let old = own_with_viewport_variants(base, variants, old_viewport);
    let new = own_with_viewport_variants(base, variants, new_viewport);
    own_visual_differs(&old, &new)
}

fn own_visual_differs(a: &Visual, b: &Visual) -> bool {
    a.background_color != b.background_color
        || (a.opacity - b.opacity).abs() > f32::EPSILON
        || (a.border_radius - b.border_radius).abs() > f32::EPSILON
        || (a.border_width - b.border_width).abs() > f32::EPSILON
        || a.border_color != b.border_color
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

/// Shared effective visual resolver (ADR-0067): inheritance → own → pseudo.
pub fn resolve_effective(
    inherited: &InheritedVisualContext,
    own: &Visual,
    pseudo: &PseudoStyles,
    interaction: &InteractionSnapshot,
    id: ElementId,
) -> Visual {
    let inherited_base = apply_text_inheritance(inherited, own);
    pseudo_state::resolve_visual(&inherited_base, pseudo, interaction, id)
}

/// Build inherited context for query at `id` (ancestor walk).
pub(crate) fn inherited_context_at(
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> InheritedVisualContext {
    InheritedVisualContext {
        ambient: ambient_defaults::ambient_at(elements, id),
        text_local: text_local_inherited_at(elements, id),
    }
}

fn text_local_inherited_at(
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> Option<TextLocalInherited> {
    let el = elements.get(&id)?;
    if el.kind != ElementKind::Text {
        return None;
    }
    let parent_id = el.parent?;
    let parent = elements.get(&parent_id)?;
    if parent.kind != ElementKind::Text {
        return None;
    }
    let parent_ctx = inherited_context_at(elements, parent_id);
    let parent_base = apply_text_inheritance(&parent_ctx, &parent.visual);
    Some(TextLocalInherited::from_inherited_base(&parent_base))
}

/// Threaded child context for top-down walks (`scene_build`, `walk_resolved`).
pub fn child_inherited_context(
    parent_ctx: &InheritedVisualContext,
    parent_kind: ElementKind,
    parent_inherited_base: &Visual,
    parent_own: &Visual,
) -> InheritedVisualContext {
    InheritedVisualContext {
        ambient: parent_ctx.ambient.merge_visual(parent_own),
        text_local: if parent_kind == ElementKind::Text {
            Some(TextLocalInherited::from_inherited_base(parent_inherited_base))
        } else {
            None
        },
    }
}
