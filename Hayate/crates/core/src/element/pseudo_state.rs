use std::collections::HashSet;

use crate::element::id::ElementId;
use crate::element::style::{StyleProp, StylePropKind};
use crate::element::tree::{Element, Visual};

mod tables {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/pseudo_state_tables.rs"
    ));
}

/// Hayate CSS の要素ローカルな擬似クラス（`:hover`、`:active`、`:focus`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PseudoState {
    Hover,
    Active,
    Focus,
}

impl PseudoState {
    pub fn from_u32(v: u32) -> Option<Self> {
        tables::pseudo_state_from_u32(v)
    }
}

/// Hayate CSS で宣言された擬似クラスごとのスタイル上書き。
#[derive(Clone, Debug, Default)]
pub struct PseudoStyles {
    pub hover: Vec<StyleProp>,
    pub active: Vec<StyleProp>,
    pub focus: Vec<StyleProp>,
}

impl PseudoStyles {
    pub fn props_mut(&mut self, state: PseudoState) -> &mut Vec<StyleProp> {
        match state {
            PseudoState::Hover => &mut self.hover,
            PseudoState::Active => &mut self.active,
            PseudoState::Focus => &mut self.focus,
        }
    }

    pub fn props(&self, state: PseudoState) -> &[StyleProp] {
        match state {
            PseudoState::Hover => &self.hover,
            PseudoState::Active => &self.active,
            PseudoState::Focus => &self.focus,
        }
    }
}

/// 実効スタイル解決時に使う、ポインタ由来のインタラクションフラグ。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InteractionSnapshot {
    pub hovered: HashSet<ElementId>,
    pub active: Option<ElementId>,
    pub focused: Option<ElementId>,
}

impl InteractionSnapshot {
    pub fn is_hovered(&self, id: ElementId) -> bool {
        self.hovered.contains(&id)
    }

    pub fn is_active(&self, id: ElementId) -> bool {
        self.active == Some(id)
    }

    pub fn is_focused(&self, id: ElementId) -> bool {
        self.focused == Some(id)
    }
}

pub fn style_prop_key(prop: &StyleProp) -> &'static str {
    match prop {
        StyleProp::BackgroundColor(_) => "background-color",
        StyleProp::Opacity(_) => "opacity",
        StyleProp::BorderRadius(_) => "border-radius",
        StyleProp::BorderWidth(_) => "border-width",
        StyleProp::BorderColor(_) => "border-color",
        StyleProp::BorderStyle(_) => "border-style",
        StyleProp::BoxShadow(_) => "box-shadow",
        StyleProp::Overflow(_) => "overflow",
        StyleProp::Width(_) => "width",
        StyleProp::Height(_) => "height",
        StyleProp::MinWidth(_) => "min-width",
        StyleProp::MinHeight(_) => "min-height",
        StyleProp::MaxWidth(_) => "max-width",
        StyleProp::MaxHeight(_) => "max-height",
        StyleProp::Display(_) => "display",
        StyleProp::FlexDirection(_) => "flex-direction",
        StyleProp::FlexWrap(_) => "flex-wrap",
        StyleProp::AlignItems(_) => "align-items",
        StyleProp::JustifyContent(_) => "justify-content",
        StyleProp::Gap(_) => "gap",
        StyleProp::Padding(_) => "padding",
        StyleProp::PaddingTop(_) => "padding-top",
        StyleProp::PaddingRight(_) => "padding-right",
        StyleProp::PaddingBottom(_) => "padding-bottom",
        StyleProp::PaddingLeft(_) => "padding-left",
        StyleProp::Margin(_) => "margin",
        StyleProp::MarginTop(_) => "margin-top",
        StyleProp::MarginRight(_) => "margin-right",
        StyleProp::MarginBottom(_) => "margin-bottom",
        StyleProp::MarginLeft(_) => "margin-left",
        StyleProp::Position(_) => "position",
        StyleProp::Top(_) => "top",
        StyleProp::Left(_) => "left",
        StyleProp::Right(_) => "right",
        StyleProp::Bottom(_) => "bottom",
        StyleProp::FlexGrow(_) => "flex-grow",
        StyleProp::FlexShrink(_) => "flex-shrink",
        StyleProp::FlexBasis(_) => "flex-basis",
        StyleProp::AlignSelf(_) => "align-self",
        StyleProp::AlignContent(_) => "align-content",
        StyleProp::GridTemplateColumns(_) => "grid-template-columns",
        StyleProp::GridTemplateRows(_) => "grid-template-rows",
        StyleProp::FontSize(_) => "font-size",
        StyleProp::FontFamily(_) => "font-family",
        StyleProp::FontWeight(_) => "font-weight",
        StyleProp::Color(_) => "color",
        StyleProp::FontStyle(_) => "font-style",
        StyleProp::TextDecoration(_) => "text-decoration",
        StyleProp::MaxLines(_) => "max-lines",
        StyleProp::TextOverflow(_) => "text-overflow",
        StyleProp::Cursor(_) => "cursor",
        StyleProp::DefaultColor(_) => "default-color",
        StyleProp::DefaultFontFamily(_) => "default-font-family",
        StyleProp::DefaultFontSize(_) => "default-font-size",
        StyleProp::DefaultFontWeight(_) => "default-font-weight",
        StyleProp::ZIndex(_) => "z-index",
        StyleProp::TransitionDuration(_) => "transition-duration",
        StyleProp::TransitionTiming(_) => "transition-timing",
    }
}

pub fn upsert_style_prop(slot: &mut Vec<StyleProp>, prop: &StyleProp) {
    let key = style_prop_key(prop);
    slot.retain(|p| style_prop_key(p) != key);
    slot.push(prop.clone());
}

/// `props` を `visual` に適用する（visual/text フィールドのみ。レイアウトプロパティはここでは無視）。
pub fn apply_visual_props(visual: &mut Visual, props: &[StyleProp], text_dirty: &mut bool) {
    for prop in props {
        if prop.is_layout() {
            continue;
        }
        super::tree::apply_visual(visual, prop, text_dirty);
    }
}

/// ベース `visual` に擬似状態の上書きを仕様の優先順位で重ねる（後勝ち）。
pub fn resolve_visual(
    base: &Visual,
    pseudo: &PseudoStyles,
    interaction: &InteractionSnapshot,
    id: ElementId,
) -> Visual {
    let mut out = base.clone();
    let mut text_dirty = false;
    for state in tables::PSEUDO_RESOLVE_ORDER {
        let active = match state {
            PseudoState::Focus => interaction.is_focused(id),
            PseudoState::Hover => interaction.is_hovered(id),
            PseudoState::Active => interaction.is_active(id),
        };
        if active {
            apply_visual_props(&mut out, pseudo.props(state), &mut text_dirty);
        }
    }
    let _ = text_dirty;
    out
}

/// `id` から root まで（root を含む）の祖先チェーンを求める。
pub(crate) fn ancestor_chain(
    elements: &std::collections::HashMap<ElementId, Element>,
    id: ElementId,
) -> Vec<ElementId> {
    let mut chain = Vec::new();
    let mut cur = Some(id);
    while let Some(node) = cur {
        if !elements.contains_key(&node) {
            break;
        }
        chain.push(node);
        cur = elements.get(&node).and_then(|e| e.parent);
    }
    chain
}

/// CSS `:hover` 集合: `deepest_hit` 自身とその全祖先。
pub(crate) fn hover_set_for_hit(
    elements: &std::collections::HashMap<ElementId, Element>,
    deepest_hit: ElementId,
) -> HashSet<ElementId> {
    ancestor_chain(elements, deepest_hit)
        .into_iter()
        .collect()
}

/// 2 つの hover 集合の差分を (entered, left) の要素 id に分ける。
pub fn diff_hover_sets(
    prev: &HashSet<ElementId>,
    next: &HashSet<ElementId>,
) -> (Vec<ElementId>, Vec<ElementId>) {
    let entered: Vec<_> = next.difference(prev).copied().collect();
    let left: Vec<_> = prev.difference(next).copied().collect();
    (entered, left)
}

/// 擬似ブロックがインラインテキストを再シェイプするプロパティを含むかどうか。
pub fn pseudo_affects_text_shaping(props: &[StyleProp]) -> bool {
    props.iter().any(|p| {
        matches!(
            p,
            StyleProp::FontSize(_)
                | StyleProp::FontFamily(_)
                | StyleProp::FontWeight(_)
                | StyleProp::FontStyle(_)
                | StyleProp::Color(_)
                | StyleProp::TextDecoration(_)
        )
    })
}

/// 擬似ブロック上で unset 可能なスタイルプロパティ種別（ベースの unset と同形）。
pub fn unset_pseudo_prop(pseudo: &mut PseudoStyles, state: PseudoState, kind: StylePropKind) {
    let props = pseudo.props_mut(state);
    props.retain(|p| match (state, kind, p) {
        (_, StylePropKind::Color, StyleProp::Color(_)) => false,
        (_, StylePropKind::FontSize, StyleProp::FontSize(_)) => false,
        (_, StylePropKind::FontFamily, StyleProp::FontFamily(_)) => false,
        (_, StylePropKind::FontWeight, StyleProp::FontWeight(_)) => false,
        _ => true,
    });
}
