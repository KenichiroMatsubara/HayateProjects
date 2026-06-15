use std::collections::HashMap;

use crate::element::ambient_defaults::AmbientDefaults;
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
pub(crate) fn own_with_viewport_variants(
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

/// Shared effective visual resolver (ADR-0067, ADR-0081): inheritance (ch1+ch2)
/// → own (base + matching viewport variants) → pseudo. Viewport-variant
/// application lives inside this single seam, so every caller (query,
/// `scene_build`, `InlineText`) shares one resolution entry instead of pre-baking
/// own.
pub fn resolve_effective(
    inherited: &InheritedVisualContext,
    own_base: &Visual,
    viewport_variants: &[(ViewportCondition, StyleProp)],
    viewport: (f32, f32),
    pseudo: &PseudoStyles,
    interaction: &InteractionSnapshot,
    id: ElementId,
) -> Visual {
    let own = own_with_viewport_variants(own_base, viewport_variants, viewport);
    let inherited_base = apply_text_inheritance(inherited, &own);
    pseudo_state::resolve_visual(&inherited_base, pseudo, interaction, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::pseudo_state::{InteractionSnapshot, PseudoStyles};
    use crate::element::style::ViewportCondition;
    use crate::element::tree::Visual;

    fn base_red() -> Visual {
        let mut v = Visual::default();
        v.background_color = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        v
    }

    fn min_width_blue_variant() -> Vec<(ViewportCondition, StyleProp)> {
        vec![(
            ViewportCondition {
                min_width: Some(768.0),
                ..Default::default()
            },
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        )]
    }

    // The shared resolver seam owns viewport-variant application (ADR-0081):
    // callers pass base own + variants + viewport, not a pre-baked own.
    #[test]
    fn resolve_effective_applies_matching_viewport_variant() {
        let ctx = InheritedVisualContext::root();
        let pseudo = PseudoStyles::default();
        let interaction = InteractionSnapshot::default();

        let above = resolve_effective(
            &ctx,
            &base_red(),
            &min_width_blue_variant(),
            (1024.0, 800.0),
            &pseudo,
            &interaction,
            ElementId::from_u64(1),
        );
        assert_eq!(
            above.background_color,
            Some(Color::new(0.0, 0.0, 1.0, 1.0)),
            "viewport at/above min-width must resolve the variant inside the seam"
        );

        let below = resolve_effective(
            &ctx,
            &base_red(),
            &min_width_blue_variant(),
            (500.0, 800.0),
            &pseudo,
            &interaction,
            ElementId::from_u64(1),
        );
        assert_eq!(
            below.background_color,
            Some(Color::new(1.0, 0.0, 0.0, 1.0)),
            "viewport below min-width must keep the base style"
        );
    }
}

/// Build the inherited context *at* `id` by folding root→`id` through the single
/// inheritance primitive ([`child_inherited_context`]). This is the backward
/// (ancestor-walk) entry — used by the query path (`element_effective_visual`)
/// and to seed a retained scene re-walk at an arbitrary patch root — but it
/// threads the exact same fold the top-down scene walk applies step by step, so
/// the two paths can never diverge on what an element inherits (#302 §1c).
pub(crate) fn inherited_context_at(
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> InheritedVisualContext {
    let Some(parent_id) = elements.get(&id).and_then(|el| el.parent) else {
        return InheritedVisualContext::root();
    };
    let Some(parent) = elements.get(&parent_id) else {
        return InheritedVisualContext::root();
    };
    let parent_ctx = inherited_context_at(elements, parent_id);
    let parent_inherited_base = apply_text_inheritance(&parent_ctx, &parent.visual);
    child_inherited_context(&parent_ctx, parent.kind, &parent_inherited_base, &parent.visual)
}

/// Fold one inheritance step: derive a child's inherited context from its
/// parent's. This is the single inheritance primitive — both the top-down scene
/// walk (`scene_build`, `inline_text`) and the backward ancestor walk
/// ([`inherited_context_at`]) thread their context through it (#302 §1c).
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
