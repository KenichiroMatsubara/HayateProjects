//! HTML Mode pseudo-state → browser CSS emitter (#177).
//!
//! Merges active pseudo patches in spec-generated priority order, applies
//! text-channel gating, and maps props through the spec-generated DOM mapper.

use std::collections::HashMap;

pub use hayate_core::PseudoState;
use hayate_core::{ElementKind, StyleProp};

use crate::generated;

mod tables {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../proto/generated/pseudo_state_tables.rs"
    ));
}

/// Per-pseudo style patches for parity fixtures and tests.
#[derive(Clone, Debug, Default)]
pub struct PseudoStylesFixture {
    pub hover: Vec<StyleProp>,
    pub active: Vec<StyleProp>,
    pub focus: Vec<StyleProp>,
}

impl PseudoStylesFixture {
    fn props(&self, state: PseudoState) -> &[StyleProp] {
        match state {
            PseudoState::Hover => &self.hover,
            PseudoState::Active => &self.active,
            PseudoState::Focus => &self.focus,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ParityInteraction {
    pub focus: bool,
    pub hover: bool,
    pub active: bool,
}

fn interaction_active(state: PseudoState, interaction: &ParityInteraction) -> bool {
    match state {
        PseudoState::Focus => interaction.focus,
        PseudoState::Hover => interaction.hover,
        PseudoState::Active => interaction.active,
    }
}

fn should_apply_prop(element_kind: ElementKind, prop: &StyleProp) -> bool {
    // Style Channel gating, generated from proto/spec (ADR-0002 Semantics Parity):
    // channel-1 text-local props only reach Text-Local Carrier kinds.
    if generated::is_text_local(prop) {
        return generated::carries_text_local(element_kind);
    }
    true
}

/// Map one Hayate CSS prop to browser CSS declarations (spec-generated extras included).
pub fn collect_style_prop_css(
    element_kind: ElementKind,
    prop: &StyleProp,
    out: &mut HashMap<String, String>,
) {
    if !should_apply_prop(element_kind, prop) {
        return;
    }
    let mut entries = Vec::new();
    generated::style_prop_css_entries(prop, &mut entries);
    for (property, value) in entries {
        out.insert(property, value);
    }
}

/// Merge active pseudo patches in priority order, then emit browser CSS properties.
pub fn resolve_pseudo_css_map(
    element_kind: ElementKind,
    pseudo: &PseudoStylesFixture,
    interaction: &ParityInteraction,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for state in tables::PSEUDO_RESOLVE_ORDER {
        if !interaction_active(state, interaction) {
            continue;
        }
        for prop in pseudo.props(state) {
            collect_style_prop_css(element_kind, prop, &mut map);
        }
    }
    map
}

/// CSS rule body for one pseudo-state patch (`property:value;...`).
pub fn pseudo_patch_rule_body(element_kind: ElementKind, props: &[StyleProp]) -> String {
    let mut map = HashMap::new();
    for prop in props {
        collect_style_prop_css(element_kind, prop, &mut map);
    }
    map.into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(property, value)| format!("{property}:{value}"))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn pseudo_state_css_suffix(state: PseudoState) -> &'static str {
    tables::pseudo_state_css_suffix(state)
}

pub fn pseudo_state_css_priority(state: PseudoState) -> u32 {
    tables::pseudo_state_css_priority(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::Color;

    #[test]
    fn single_hover_background() {
        let pseudo = PseudoStylesFixture {
            hover: vec![StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))],
            ..Default::default()
        };
        let map = resolve_pseudo_css_map(
            ElementKind::View,
            &pseudo,
            &ParityInteraction {
                hover: true,
                ..Default::default()
            },
        );
        assert_eq!(map.get("background-color").map(String::as_str), Some("#0000ff"));
    }
}
