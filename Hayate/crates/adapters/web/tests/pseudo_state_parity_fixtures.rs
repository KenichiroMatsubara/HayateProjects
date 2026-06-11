//! Cross-language pseudo-state parity corpus (#176 / #177): HTML Mode DOM emitter.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use hayate_adapter_web::pseudo_style_dom::{
    resolve_pseudo_css_map, ParityInteraction, PseudoStylesFixture,
};
use hayate_core::{Color, ElementKind, StyleProp};
use serde_json::Value;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../proto/spec/fixtures/pseudo_state_parity.json")
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
            "backgroundColor" => StyleProp::BackgroundColor(parse_hex_color(value.as_str().unwrap())),
            "borderWidth" => StyleProp::BorderWidth(value.as_f64().unwrap() as f32),
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

fn element_kind_from_str(s: &str) -> ElementKind {
    match s {
        "view" => ElementKind::View,
        "text" => ElementKind::Text,
        other => panic!("unknown elementKind: {other}"),
    }
}

fn pseudo_styles_from_fixture(pseudo: &Value) -> PseudoStylesFixture {
    let mut fixture = PseudoStylesFixture::default();
    let obj = pseudo.as_object().expect("pseudo object");
    for (key, patch) in obj {
        let props = style_props_from_patch(patch.as_object().expect("pseudo patch"));
        match key.as_str() {
            ":hover" => fixture.hover = props,
            ":active" => fixture.active = props,
            ":focus" => fixture.focus = props,
            other => panic!("unknown pseudo key: {other}"),
        }
    }
    fixture
}

fn interaction_from_fixture(interaction: &Value) -> ParityInteraction {
    ParityInteraction {
        focus: interaction
            .get("focus")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        hover: interaction
            .get("hover")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        active: interaction
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn expected_property_map(fixture: &Value) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for prop in fixture["expected"]["properties"].as_array().unwrap() {
        map.insert(
            prop["property"].as_str().unwrap().to_string(),
            prop["value"].as_str().unwrap().to_string(),
        );
    }
    map
}

#[test]
fn pseudo_state_parity_corpus_html_mode() {
    for fixture in load_fixtures() {
        let name = fixture["name"].as_str().unwrap_or("?");
        let element_kind = element_kind_from_str(fixture["elementKind"].as_str().unwrap());
        let pseudo = pseudo_styles_from_fixture(&fixture["pseudo"]);
        let interaction = interaction_from_fixture(&fixture["interaction"]);

        let actual = resolve_pseudo_css_map(element_kind, &pseudo, &interaction);
        let expected = expected_property_map(&fixture);

        assert_eq!(
            actual.len(),
            expected.len(),
            "{name}: property count mismatch\n  actual: {actual:?}\n  expected: {expected:?}"
        );
        for (property, value) in &expected {
            assert_eq!(
                actual.get(property),
                Some(value),
                "{name}: {property}"
            );
        }
    }
}

#[test]
fn corpus_catches_dropped_dom_extras() {
    let fixture = load_fixtures()
        .into_iter()
        .find(|f| f["name"] == "hover_border_width_dom_extra")
        .expect("hover_border_width_dom_extra fixture");
    let element_kind = element_kind_from_str(fixture["elementKind"].as_str().unwrap());
    let pseudo = pseudo_styles_from_fixture(&fixture["pseudo"]);
    let interaction = interaction_from_fixture(&fixture["interaction"]);

    let mut actual = resolve_pseudo_css_map(element_kind, &pseudo, &interaction);
    actual.remove("border-style");

    let expected = expected_property_map(&fixture);
    assert!(actual.get("border-style").is_none());
    assert_ne!(actual, expected, "dropped border-style extra must diverge from corpus");
}

#[test]
fn corpus_catches_flipped_pseudo_priority() {
    let fixture = load_fixtures()
        .into_iter()
        .find(|f| f["name"] == "hover_active_priority_active_wins")
        .expect("hover_active_priority_active_wins fixture");
    let element_kind = element_kind_from_str(fixture["elementKind"].as_str().unwrap());
    let pseudo = pseudo_styles_from_fixture(&fixture["pseudo"]);

    // Wrong band order: hover applied after active.
    let mut actual = HashMap::new();
    for props in [&pseudo.active, &pseudo.hover] {
        for prop in props {
            hayate_adapter_web::pseudo_style_dom::collect_style_prop_css(
                element_kind,
                prop,
                &mut actual,
            );
        }
    }

    let expected = expected_property_map(&fixture);
    assert_ne!(
        actual.get("background-color"),
        expected.get("background-color"),
        "flipped priority must diverge from corpus"
    );
    assert_eq!(actual.get("background-color").map(String::as_str), Some("#00ff00"));
}
