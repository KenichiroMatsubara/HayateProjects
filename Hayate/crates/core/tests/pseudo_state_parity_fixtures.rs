//! 言語間の擬似状態パリティコーパス: `resolve_visual` と DOM emitter の一致を検証する。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use hayate_core::element::pseudo_state::{
    self, apply_visual_props, InteractionSnapshot, PseudoState, PseudoStyles,
};
use hayate_core::element::tree::Visual;
use hayate_core::{BorderStyleValue, Color, ElementId, ElementKind, StyleProp};
use serde_json::Value;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proto/spec/fixtures/pseudo_state_parity.json")
}

fn load_fixtures() -> Vec<Value> {
    let text = fs::read_to_string(fixture_path()).expect("read pseudo_state_parity.json");
    serde_json::from_str(&text).expect("parse pseudo_state_parity.json")
}

fn parse_hex_color(s: &str) -> Color {
    let hex = s.strip_prefix('#').unwrap_or(s);
    assert_eq!(hex.len(), 6, "expected #rrggbb color, got {s}");
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap() as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap() as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap() as f64 / 255.0;
    Color::new(r, g, b, 1.0)
}

fn style_props_from_patch(obj: &serde_json::Map<String, Value>) -> Vec<StyleProp> {
    let mut props = Vec::new();
    for (key, value) in obj {
        let prop = match key.as_str() {
            "backgroundColor" => {
                StyleProp::BackgroundColor(parse_hex_color(value.as_str().unwrap()))
            }
            "borderWidth" => StyleProp::BorderWidth(value.as_f64().unwrap() as f32),
            "borderStyle" => StyleProp::BorderStyle(parse_border_style(value.as_str().unwrap())),
            "borderRadius" => StyleProp::BorderRadius(value.as_f64().unwrap() as f32),
            "opacity" => StyleProp::Opacity(value.as_f64().unwrap() as f32),
            "color" => StyleProp::Color(parse_hex_color(value.as_str().unwrap())),
            "fontSize" => StyleProp::FontSize(value.as_f64().unwrap() as f32),
            other => panic!("unsupported patch key in fixture: {other}"),
        };
        props.push(prop);
    }
    props
}

fn parse_border_style(s: &str) -> BorderStyleValue {
    match s {
        "none" => BorderStyleValue::None,
        "solid" => BorderStyleValue::Solid,
        "dashed" => BorderStyleValue::Dashed,
        other => panic!("unknown border-style: {other}"),
    }
}

fn border_style_css(value: BorderStyleValue) -> &'static str {
    match value {
        BorderStyleValue::None => "none",
        BorderStyleValue::Solid => "solid",
        BorderStyleValue::Dashed => "dashed",
    }
}

fn element_kind_from_str(s: &str) -> ElementKind {
    match s {
        "view" => ElementKind::View,
        "text" => ElementKind::Text,
        other => panic!("unknown elementKind: {other}"),
    }
}

fn pseudo_styles_from_fixture(pseudo: &Value) -> PseudoStyles {
    let mut styles = PseudoStyles::default();
    let obj = pseudo.as_object().expect("pseudo object");
    for (key, patch) in obj {
        let state = match key.as_str() {
            ":hover" => PseudoState::Hover,
            ":active" => PseudoState::Active,
            ":focus" => PseudoState::Focus,
            other => panic!("unknown pseudo key: {other}"),
        };
        let props = style_props_from_patch(patch.as_object().expect("pseudo patch"));
        let slot = styles.props_mut(state);
        for prop in &props {
            pseudo_state::upsert_style_prop(slot, prop);
        }
    }
    styles
}

fn interaction_from_fixture(interaction: &Value, id: ElementId) -> InteractionSnapshot {
    let mut snap = InteractionSnapshot::default();
    if interaction
        .get("hover")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        snap.hovered.insert(id);
    }
    if interaction
        .get("active")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        snap.active = Some(id);
    }
    if interaction
        .get("focus")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        snap.focused = Some(id);
    }
    snap
}

fn color_to_hex(c: Color) -> String {
    let r = (c.r * 255.0).round() as u8;
    let g = (c.g * 255.0).round() as u8;
    let b = (c.b * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// DOM のテキストチャネルのゲーティングを再現する: text 固有のキーは text 要素でのみ観測できる。
fn visual_to_parity_map(visual: &Visual, element_kind: ElementKind) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(c) = visual.background_color {
        map.insert("background-color".into(), color_to_hex(c));
    }
    if (visual.opacity - 1.0).abs() > f32::EPSILON {
        map.insert("opacity".into(), format!("{}", visual.opacity));
    }
    if visual.border_radius > f32::EPSILON {
        map.insert(
            "border-radius".into(),
            format!("{}px", visual.border_radius),
        );
    }
    if visual.border_width > f32::EPSILON {
        map.insert("border-width".into(), format!("{}px", visual.border_width));
    }
    if visual.border_style != BorderStyleValue::None {
        map.insert(
            "border-style".into(),
            border_style_css(visual.border_style).into(),
        );
    }
    if element_kind == ElementKind::Text {
        if let Some(c) = visual.text_color {
            map.insert("color".into(), color_to_hex(c));
        }
        if let Some(fs) = visual.font_size {
            map.insert("font-size".into(), format!("{}px", fs));
        }
    }
    map
}

fn expected_property_map(fixture: &Value, side: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for prop in fixture["expected"]["properties"].as_array().unwrap() {
        if side == "rust" && prop.get("domOnly").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        map.insert(
            prop["property"].as_str().unwrap().to_string(),
            prop["value"].as_str().unwrap().to_string(),
        );
    }
    map
}

#[test]
fn pseudo_state_parity_corpus_resolve_visual() {
    let id = ElementId::from_u64(1);
    for fixture in load_fixtures() {
        let name = fixture["name"].as_str().unwrap_or("?");
        let element_kind = element_kind_from_str(fixture["elementKind"].as_str().unwrap());
        let pseudo = pseudo_styles_from_fixture(&fixture["pseudo"]);
        let interaction = interaction_from_fixture(&fixture["interaction"], id);
        let base = Visual::default();

        let resolved = pseudo_state::resolve_visual(&base, &pseudo, &interaction, id);
        let actual = visual_to_parity_map(&resolved, element_kind);
        let expected = expected_property_map(&fixture, "rust");

        assert_eq!(
            actual.len(),
            expected.len(),
            "{name}: property count mismatch\n  actual: {actual:?}\n  expected: {expected:?}"
        );
        for (property, value) in &expected {
            assert_eq!(actual.get(property), Some(value), "{name}: {property}");
        }
    }
}

#[test]
fn corpus_catches_flipped_pseudo_priority() {
    let fixture = load_fixtures()
        .into_iter()
        .find(|f| f["name"] == "hover_active_priority_active_wins")
        .expect("hover_active_priority_active_wins fixture");
    let pseudo = pseudo_styles_from_fixture(&fixture["pseudo"]);
    let mut out = Visual::default();
    let mut text_dirty = false;
    // 誤った band 順序: hover を active の後に適用する。
    apply_visual_props(&mut out, pseudo.props(PseudoState::Active), &mut text_dirty);
    apply_visual_props(&mut out, pseudo.props(PseudoState::Hover), &mut text_dirty);

    let actual = color_to_hex(out.background_color.unwrap());
    let expected = expected_property_map(&fixture, "rust")
        .get("background-color")
        .cloned()
        .unwrap();
    assert_ne!(
        actual, expected,
        "flipped priority must diverge from corpus"
    );
    assert_eq!(actual, "#00ff00");
}
