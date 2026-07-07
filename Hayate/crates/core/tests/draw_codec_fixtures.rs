//! draw display list の codec fixture テスト（#724 / ADR-0142）。
//!
//! `proto/spec/fixtures/draw_encode.json` は TS encode（Tsubame 側 recorder）と
//! Rust decode（本テスト）が共有する正本 fixture。TS 側が semantic commands を
//! encode した wire と、Rust 側が wire を decode した commands が、同じ fixture の
//! 両面で一致することで encode/decode drift を機械検出する。

use hayate_core::wire::{decode_draw_list, DrawCommand, DrawPaint, PathVerb};

fn fixture_path() -> String {
    format!(
        "{}/../../proto/spec/fixtures/draw_encode.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn f32_at(v: &serde_json::Value) -> f32 {
    v.as_f64().expect("number") as f32
}

/// fixture の semantic `commands` を decode 期待値（`Vec<DrawCommand>`）へ解釈する。
fn expected_commands(commands: &[serde_json::Value]) -> Vec<DrawCommand> {
    let mut out = Vec::new();
    let mut verbs: Vec<PathVerb> = Vec::new();
    for command in commands {
        match command["op"].as_str().expect("op name") {
            "moveTo" => verbs.push(PathVerb::MoveTo {
                x: f32_at(&command["x"]),
                y: f32_at(&command["y"]),
            }),
            "lineTo" => verbs.push(PathVerb::LineTo {
                x: f32_at(&command["x"]),
                y: f32_at(&command["y"]),
            }),
            "close" => verbs.push(PathVerb::Close),
            "quadraticTo" => verbs.push(PathVerb::QuadraticTo {
                cx: f32_at(&command["cx"]),
                cy: f32_at(&command["cy"]),
                x: f32_at(&command["x"]),
                y: f32_at(&command["y"]),
            }),
            "cubicTo" => verbs.push(PathVerb::CubicTo {
                c1x: f32_at(&command["c1x"]),
                c1y: f32_at(&command["c1y"]),
                c2x: f32_at(&command["c2x"]),
                c2y: f32_at(&command["c2y"]),
                x: f32_at(&command["x"]),
                y: f32_at(&command["y"]),
            }),
            "arcTo" => verbs.push(PathVerb::ArcTo {
                x1: f32_at(&command["x1"]),
                y1: f32_at(&command["y1"]),
                x2: f32_at(&command["x2"]),
                y2: f32_at(&command["y2"]),
                radius: f32_at(&command["radius"]),
            }),
            "rect" => verbs.push(PathVerb::Rect {
                x: f32_at(&command["x"]),
                y: f32_at(&command["y"]),
                width: f32_at(&command["width"]),
                height: f32_at(&command["height"]),
            }),
            "rrect" => verbs.push(PathVerb::Rrect {
                x: f32_at(&command["x"]),
                y: f32_at(&command["y"]),
                width: f32_at(&command["width"]),
                height: f32_at(&command["height"]),
                rx: f32_at(&command["rx"]),
                ry: f32_at(&command["ry"]),
            }),
            "oval" => verbs.push(PathVerb::Oval {
                x: f32_at(&command["x"]),
                y: f32_at(&command["y"]),
                width: f32_at(&command["width"]),
                height: f32_at(&command["height"]),
            }),
            "circle" => verbs.push(PathVerb::Circle {
                cx: f32_at(&command["cx"]),
                cy: f32_at(&command["cy"]),
                radius: f32_at(&command["radius"]),
            }),
            "fill" => {
                let mut paint = DrawPaint::default();
                if let Some(color) = command["paint"]["color"].as_array() {
                    paint.color = [
                        f32_at(&color[0]),
                        f32_at(&color[1]),
                        f32_at(&color[2]),
                        f32_at(&color[3]),
                    ];
                }
                if let Some(rule) = command["paint"]["fillRule"].as_f64() {
                    paint.fill_rule = rule as f32;
                }
                out.push(DrawCommand::FillPath {
                    verbs: std::mem::take(&mut verbs),
                    paint,
                });
            }
            other => panic!("fixture uses unknown draw command {other}"),
        }
    }
    out
}

// 共有 fixture の wire を Rust decode し、semantic commands と一致することを検証する
// （TS 側は同じ fixture で encode 一致を検証する = TS encode ↔ Rust decode roundtrip）。
#[test]
fn draw_encode_fixtures_decode_to_expected_commands() {
    let text = std::fs::read_to_string(fixture_path()).expect("read draw_encode.json");
    let fixtures: Vec<serde_json::Value> = serde_json::from_str(&text).expect("parse fixture");
    assert!(!fixtures.is_empty(), "draw_encode.json must not be empty");

    for fixture in &fixtures {
        let name = fixture["name"].as_str().expect("fixture name");
        let wire: Vec<f32> = fixture["wire"]
            .as_array()
            .expect("wire array")
            .iter()
            .map(f32_at)
            .collect();
        let decoded = decode_draw_list(&wire)
            .unwrap_or_else(|e| panic!("fixture {name}: decode failed: {e}"));
        let expected = expected_commands(fixture["commands"].as_array().expect("commands"));
        assert_eq!(decoded, expected, "fixture {name}");
    }
}

// 空 display list は空コマンド列（穴のない縮退ケース）。
#[test]
fn empty_draw_list_decodes_to_no_commands() {
    assert_eq!(decode_draw_list(&[]).unwrap(), Vec::<DrawCommand>::new());
}

// 未知 op / 途切れた payload はエラー（黙って部分適用しない）。
#[test]
fn malformed_draw_list_is_an_error() {
    assert!(decode_draw_list(&[99.0]).is_err(), "unknown op must error");
    assert!(
        decode_draw_list(&[0.0, 1.0]).is_err(),
        "truncated MOVE_TO payload must error"
    );
    assert!(
        decode_draw_list(&[3.0, 5.0, 0.0, 1.0]).is_err(),
        "truncated FILL paint packet must error"
    );
}
