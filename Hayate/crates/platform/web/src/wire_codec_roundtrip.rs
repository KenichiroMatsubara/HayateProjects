//! C1: フィクスチャの wire → decode → encode ラウンドトリップ（ADR-0055）。

#[cfg(test)]
mod tests {
    use crate::generated::{decode_style_packet, encode_op, encode_style_packet, parse_next_op};
    use std::fs;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../proto/spec/fixtures")
            .join(name)
    }

    #[test]
    fn style_wire_decode_encode_roundtrip() {
        let text =
            fs::read_to_string(fixture_path("style_encode.json")).expect("read style fixtures");
        let fixtures: Vec<serde_json::Value> =
            serde_json::from_str(&text).expect("parse style fixtures");

        for fixture in fixtures {
            let name = fixture["name"].as_str().unwrap_or("?");
            let wire: Vec<f32> = fixture["wire"]
                .as_array()
                .unwrap_or_else(|| panic!("{name}: missing wire"))
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();

            let props = decode_style_packet(&wire).unwrap_or_else(|e| {
                panic!("{name}: decode failed: {:?}", e);
            });
            let mut encoded = Vec::new();
            encode_style_packet(&mut encoded, &props);
            assert_eq!(encoded, wire, "{name}: encode roundtrip mismatch");
        }
    }

    #[test]
    fn ops_wire_parse_encode_roundtrip() {
        let text = fs::read_to_string(fixture_path("ops_encode.json")).expect("read op fixtures");
        let fixtures: Vec<serde_json::Value> =
            serde_json::from_str(&text).expect("parse op fixtures");

        for fixture in fixtures {
            let name = fixture["name"].as_str().unwrap_or("?");
            let wire: Vec<f64> = fixture["wire"]
                .as_array()
                .unwrap_or_else(|| panic!("{name}: missing wire"))
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();

            let mut i = 0usize;
            let mut ops = Vec::new();
            while i < wire.len() {
                let (op, next) = parse_next_op(&wire, i).unwrap_or_else(|e| panic!("{name}: {e}"));
                ops.push(op);
                i = next;
            }

            let mut encoded = Vec::new();
            for op in &ops {
                encode_op(&mut encoded, op);
            }
            assert_eq!(encoded, wire, "{name}: op encode roundtrip mismatch");
        }
    }
}
