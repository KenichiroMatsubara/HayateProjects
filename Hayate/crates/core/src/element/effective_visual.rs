use std::collections::HashMap;

use crate::element::ambient_defaults::AmbientDefaults;
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::pseudo_state::{self, InteractionSnapshot, PseudoStyles};
use crate::element::style::{FontStyleValue, StyleProp, TextDecorationValue, ViewportCondition};
use crate::element::tree::{apply_visual, Element, Visual};
use crate::color::Color;

/// ch1 の text→text 継承テキストスタイルフィールド（ADR-0065）。
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

/// 実効ビジュアル解決のための継承コンテキスト（ADR-0067）。
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

/// `own` の未設定 text/visual フィールドに ch1 + ch2 の継承を適用する。
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

/// 一致するビューポートバリアントを `base` のコピーに適用する（ADR-0081）。
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

/// 共有の実効ビジュアル解決器（ADR-0067, ADR-0081）。継承（ch1+ch2）→ own（base +
/// 一致するビューポートバリアント）→ pseudo の順。ビューポートバリアントの適用はこの
/// 単一の継ぎ目の内側で行うため、全呼び出し元（query, `scene_build`, `InlineText`）は
/// own を事前焼き込みせず単一の解決入口を共有する。
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

    // 共有解決器の継ぎ目がビューポートバリアント適用を担う（ADR-0081）。呼び出し元は
    // 事前焼き込み済みの own ではなく base own + variants + viewport を渡す。
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

/// 単一の継承プリミティブ（[`child_inherited_context`]）で root→`id` を畳み込み、
/// `id` における継承コンテキストを構築する。これは後方（祖先 walk）の入口で、query 経路
/// （`element_effective_visual`）や任意のパッチ根での保持シーン再 walk の初期化に使う。
/// トップダウンのシーン walk が段階的に適用するのと全く同じ畳み込みを通すため、ある要素が
/// 何を継承するかで両経路が食い違うことはない。
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

/// 継承を 1 段畳み込む。親の継承コンテキストから子のものを導く。これが単一の継承
/// プリミティブで、トップダウンのシーン walk（`scene_build`, `inline_text`）も後方の
/// 祖先 walk（[`inherited_context_at`]）も、ここを通してコンテキストを伝播する。
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
