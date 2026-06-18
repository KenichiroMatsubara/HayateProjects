//! Cross-language `user-select` parity corpus (ADR-0108): the HTML Mode resolver
//! must agree with the Tsubame DOM Renderer (`resolveUserSelect`) on what each
//! element-kind default + explicit `user-select` maps to. Both sides read the
//! single source `proto/spec/fixtures/user_select_parity.json` (ADR-0070).

use std::fs;
use std::path::PathBuf;

use hayate_adapter_web::user_select::resolve_user_select;
use hayate_core::{ElementKind, UserSelectValue};
use serde_json::Value;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../proto/spec/fixtures/user_select_parity.json")
}

fn load_fixtures() -> Vec<Value> {
    let text = fs::read_to_string(fixture_path()).expect("read user_select_parity.json");
    serde_json::from_str(&text).expect("parse user_select_parity.json")
}

fn element_kind_from_str(s: &str) -> ElementKind {
    match s {
        "view" => ElementKind::View,
        "text" => ElementKind::Text,
        "image" => ElementKind::Image,
        "button" => ElementKind::Button,
        "text-input" => ElementKind::TextInput,
        "scroll-view" => ElementKind::ScrollView,
        other => panic!("unknown elementKind: {other}"),
    }
}

fn user_select_from_fixture(value: &Value) -> Option<UserSelectValue> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(match s.as_str() {
            "text" => UserSelectValue::Text,
            "none" => UserSelectValue::None,
            "contains" => UserSelectValue::Contains,
            other => panic!("unknown userSelect value: {other}"),
        }),
        other => panic!("unexpected userSelect value: {other}"),
    }
}

#[test]
fn user_select_parity_corpus_html_mode() {
    for fixture in load_fixtures() {
        let name = fixture["name"].as_str().unwrap_or("?");
        let kind = element_kind_from_str(fixture["elementKind"].as_str().unwrap());
        let user_select = user_select_from_fixture(&fixture["userSelect"]);
        let expected = fixture["expected"].as_str().unwrap();

        assert_eq!(
            resolve_user_select(kind, user_select),
            expected,
            "{name}: kind={kind:?} userSelect={user_select:?}"
        );
    }
}
