//! C5: delivery wire fixtures — event → encode_event → wire (ADR-0055).

#[cfg(test)]
mod tests {
    use crate::generated::{encode_event_wire, EventWireValue};
    use hayate_core::{ElementId, Event};
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../proto/spec/fixtures/delivery_encode.json")
    }

    fn load_fixtures() -> Vec<Value> {
        let text = fs::read_to_string(fixture_path()).expect("read delivery_encode.json");
        serde_json::from_str(&text).expect("parse delivery_encode.json")
    }

    fn event_from_fixture(kind: &str, fields: &Value) -> Event {
        let id = |key: &str| ElementId::from_u64(fields[key].as_u64().expect("element_id"));
        match kind {
            "click" => Event::Click {
                target_id: id("target_id"),
                x: fields["x"].as_f64().unwrap() as f32,
                y: fields["y"].as_f64().unwrap() as f32,
            },
            "focus" => Event::Focus {
                target_id: id("target_id"),
            },
            "blur" => Event::Blur {
                target_id: id("target_id"),
            },
            "text_input" => Event::TextInput {
                target_id: id("target_id"),
                text: fields["text"].as_str().unwrap().to_string(),
            },
            "composition_start" => Event::CompositionStart {
                target_id: id("target_id"),
                text: fields["text"].as_str().unwrap().to_string(),
            },
            "composition_update" => Event::CompositionUpdate {
                target_id: id("target_id"),
                text: fields["text"].as_str().unwrap().to_string(),
            },
            "composition_end" => Event::CompositionEnd {
                target_id: id("target_id"),
                text: fields["text"].as_str().unwrap().to_string(),
            },
            "scroll" => Event::Scroll {
                target_id: id("target_id"),
                delta_x: fields["delta_x"].as_f64().unwrap() as f32,
                delta_y: fields["delta_y"].as_f64().unwrap() as f32,
            },
            "resize" => Event::Resize {
                width: fields["width"].as_f64().unwrap() as f32,
                height: fields["height"].as_f64().unwrap() as f32,
            },
            "active_end" => Event::ActiveEnd {
                target_id: id("target_id"),
            },
            "hover_enter" => Event::HoverEnter {
                target_id: id("target_id"),
            },
            "hover_leave" => Event::HoverLeave {
                target_id: id("target_id"),
            },
            "key_down" => Event::KeyDown {
                target_id: id("target_id"),
                key: fields["key"].as_str().unwrap().to_string(),
                modifiers: fields["modifiers"].as_u64().unwrap() as u32,
            },
            "active_start" => Event::ActiveStart {
                target_id: id("target_id"),
            },
            "pointer_move" => Event::PointerMove {
                x: fields["x"].as_f64().unwrap() as f32,
                y: fields["y"].as_f64().unwrap() as f32,
            },
            "fetch_font" => Event::FetchFont {
                family: fields["family"].as_str().unwrap().to_string(),
            },
            other => panic!("unknown fixture kind: {other}"),
        }
    }

    fn wire_value_to_json(atom: EventWireValue) -> Value {
        match atom {
            EventWireValue::Number(n) => {
                if (n - n.round()).abs() < f64::EPSILON {
                    Value::from(n as i64)
                } else {
                    serde_json::Number::from_f64(n)
                        .map(Value::Number)
                        .unwrap_or_else(|| Value::from(n))
                }
            }
            EventWireValue::Text(s) => Value::String(s),
        }
    }

    #[test]
    fn delivery_wire_encode_matches_fixtures() {
        for fixture in load_fixtures() {
            let name = fixture["name"].as_str().unwrap_or("?");
            let kind = fixture["kind"].as_str().expect("kind");
            let fields = &fixture["fields"];
            let expected_wire = fixture["wire"].as_array().expect("wire");

            let event = event_from_fixture(kind, fields);
            let wire: Vec<Value> = encode_event_wire(&event)
                .into_iter()
                .map(wire_value_to_json)
                .collect();

            assert_eq!(
                wire,
                *expected_wire,
                "{name}: encode_event_wire mismatch"
            );
        }
    }
}
