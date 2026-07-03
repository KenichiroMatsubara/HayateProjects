//! `Hayate/proto/spec/enums.json` ＋ `style_tags.json` を読み、Hayabusa の `<style>` DSL が
//! 対応する enum 語彙（[`ENUM_KEYWORDS`](../src/lib.rs)）を生成する（ADR-0011）。
//!
//! JSON パースはこのファイルにしか存在しない、最小限の自前パーサ（`.hybs` パーサ・式パーサと
//! 同じ手組みの流儀・ADR-0001/0004）。`serde_json` 等の外部依存は持ち込まない。

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let spec_dir = Path::new(&manifest_dir).join("../../Hayate/proto/spec");
    let enums_path = spec_dir.join("enums.json");
    let style_tags_path = spec_dir.join("style_tags.json");

    println!("cargo:rerun-if-changed={}", enums_path.display());
    println!("cargo:rerun-if-changed={}", style_tags_path.display());

    let enums_src = fs::read_to_string(&enums_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", enums_path.display()));
    let style_tags_src = fs::read_to_string(&style_tags_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", style_tags_path.display()));

    let enums = json::parse(&enums_src);
    let style_tags = json::parse(&style_tags_src);

    let out = generate(&enums, &style_tags);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    fs::write(Path::new(&out_dir).join("enum_keywords_generated.rs"), out)
        .expect("failed to write enum_keywords_generated.rs");
}

/// Hayabusa が現時点で対応する style tag（ADR-0010 のスコープ）。
/// `(style_tags.json の name, その enum 型が使う enums.json の name, 生成する Rust 型名,
/// 対応済みの値の allow-list)`。
///
/// Rust 型名は `style.rs` が既に使っている短い名前（`Align` / `Justify`）を保つための明示的な
/// 指定で、`pascal_case(enum_key)` の機械変換（`AlignItems` / `JustifyContent`）とは意図的に
/// 分けている——型名1つを選ぶだけの話で語彙ではないため、ここで手で選んでよい。
///
/// 値の allow-list は「まだ実装していない値を DSL に露出させない」ための明示的な部分集合選択で、
/// 語彙の二重管理ではない（ADR-0011）。対応範囲を広げるときはここを足す。
const SUPPORTED_STYLE_TAGS: &[(&str, &str, &str, &[&str])] = &[
    ("DISPLAY", "display", "Display", &["flex", "block", "none"]),
    (
        "FLEX_DIRECTION",
        "flex_direction",
        "FlexDirection",
        &["row", "column"],
    ),
    (
        "ALIGN_ITEMS",
        "align_items",
        "Align",
        &["flex_start", "flex_end", "center", "stretch"],
    ),
    (
        "JUSTIFY_CONTENT",
        "justify_content",
        "Justify",
        &[
            "flex_start",
            "flex_end",
            "center",
            "space_between",
            "space_around",
            "space_evenly",
        ],
    ),
];

fn generate(enums: &json::Value, style_tags: &json::Value) -> String {
    let mut out = String::new();
    out.push_str("// 自動生成ファイル（Hayabusa/style-vocab/build.rs） — 手動で編集しないこと\n");
    out.push_str("// 生成元: Hayate/proto/spec/{enums.json, style_tags.json}（ADR-0011）\n\n");
    out.push_str("pub const ENUM_KEYWORDS: &[EnumSpec] = &[\n");

    for (tag_name, enum_key, rust_type, allowed_values) in SUPPORTED_STYLE_TAGS {
        let tag = find_by_name(style_tags, tag_name)
            .unwrap_or_else(|| panic!("style_tags.json is missing an entry named `{tag_name}`"));
        let dom_prop = tag
            .get("domCss")
            .and_then(|d| d.get("property"))
            .and_then(json::Value::as_str)
            .unwrap_or_else(|| panic!("`{tag_name}` in style_tags.json has no domCss.property"));

        let values = enum_value_names(enums, enum_key)
            .unwrap_or_else(|| panic!("enums.json is missing an enum named `{enum_key}`"));

        out.push_str(&format!(
            "    EnumSpec {{ prop: {dom_prop:?}, enum_name: {rust_type:?}, variants: &[\n"
        ));
        for value_name in &values {
            if !allowed_values.contains(&value_name.as_str()) {
                continue;
            }
            out.push_str(&format!(
                "        ({:?}, {:?}),\n",
                kebab_case(value_name),
                pascal_case(value_name)
            ));
        }
        out.push_str("    ] },\n");
    }

    out.push_str("];\n");
    out
}

/// JSON配列の中から `"name"` フィールドが一致する最初のオブジェクトを探す。
fn find_by_name<'a>(array: &'a json::Value, name: &str) -> Option<&'a json::Value> {
    array
        .as_array()?
        .iter()
        .find(|entry| entry.get("name").and_then(json::Value::as_str) == Some(name))
}

/// `enums.json` の中から `name` が一致する enum を探し、その `values[].name` を宣言順で返す。
fn enum_value_names(enums: &json::Value, name: &str) -> Option<Vec<String>> {
    let entry = find_by_name(enums, name)?;
    let values = entry.get("values")?.as_array()?;
    Some(
        values
            .iter()
            .filter_map(|v| v.get("name").and_then(json::Value::as_str))
            .map(str::to_string)
            .collect(),
    )
}

/// `flex_start` → `flex-start`（Tsubame の generator と同じ機械変換・ADR-0011）。
fn kebab_case(snake: &str) -> String {
    snake.replace('_', "-")
}

/// `flex_start` → `FlexStart`。
fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// 最小限の JSON パーサ（この用途に必要な範囲のみ）。`.hybs` パーサ・式パーサと同じく、
/// 外部依存を持ち込まず手組みする（ADR-0001 / ADR-0004 の流儀）。
mod json {
    #[allow(dead_code)] // Bool/Number are part of a general-purpose JSON value, unused by this build.rs
    pub enum Value {
        Null,
        Bool(bool),
        Number(f64),
        Str(String),
        Array(Vec<Value>),
        Object(Vec<(String, Value)>),
    }

    impl Value {
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                _ => None,
            }
        }

        pub fn as_array(&self) -> Option<&[Value]> {
            match self {
                Value::Array(a) => Some(a),
                _ => None,
            }
        }

        /// オブジェクトのフィールドを名前で引く（配列を線形探索・要素数は小さいので十分）。
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
    }

    pub fn parse(input: &str) -> Value {
        let chars: Vec<char> = input.chars().collect();
        let mut pos = 0;
        let value = parse_value(&chars, &mut pos);
        skip_ws(&chars, &mut pos);
        assert!(
            pos == chars.len(),
            "trailing content after top-level JSON value"
        );
        value
    }

    fn skip_ws(chars: &[char], pos: &mut usize) {
        while *pos < chars.len() && chars[*pos].is_whitespace() {
            *pos += 1;
        }
    }

    fn parse_value(chars: &[char], pos: &mut usize) -> Value {
        skip_ws(chars, pos);
        match chars.get(*pos) {
            Some('{') => parse_object(chars, pos),
            Some('[') => parse_array(chars, pos),
            Some('"') => Value::Str(parse_string(chars, pos)),
            Some('t') => {
                *pos += 4;
                Value::Bool(true)
            }
            Some('f') => {
                *pos += 5;
                Value::Bool(false)
            }
            Some('n') => {
                *pos += 4;
                Value::Null
            }
            Some(_) => parse_number(chars, pos),
            None => panic!("unexpected end of JSON input"),
        }
    }

    fn parse_object(chars: &[char], pos: &mut usize) -> Value {
        *pos += 1; // `{`
        let mut entries = Vec::new();
        skip_ws(chars, pos);
        if chars.get(*pos) == Some(&'}') {
            *pos += 1;
            return Value::Object(entries);
        }
        loop {
            skip_ws(chars, pos);
            let key = parse_string(chars, pos);
            skip_ws(chars, pos);
            assert_eq!(chars.get(*pos), Some(&':'), "expected `:` in JSON object");
            *pos += 1;
            let value = parse_value(chars, pos);
            entries.push((key, value));
            skip_ws(chars, pos);
            match chars.get(*pos) {
                Some(',') => *pos += 1,
                Some('}') => {
                    *pos += 1;
                    break;
                }
                _ => panic!("expected `,` or `}}` in JSON object"),
            }
        }
        Value::Object(entries)
    }

    fn parse_array(chars: &[char], pos: &mut usize) -> Value {
        *pos += 1; // `[`
        let mut items = Vec::new();
        skip_ws(chars, pos);
        if chars.get(*pos) == Some(&']') {
            *pos += 1;
            return Value::Array(items);
        }
        loop {
            let value = parse_value(chars, pos);
            items.push(value);
            skip_ws(chars, pos);
            match chars.get(*pos) {
                Some(',') => *pos += 1,
                Some(']') => {
                    *pos += 1;
                    break;
                }
                _ => panic!("expected `,` or `]` in JSON array"),
            }
        }
        Value::Array(items)
    }

    fn parse_string(chars: &[char], pos: &mut usize) -> String {
        assert_eq!(
            chars.get(*pos),
            Some(&'"'),
            "expected `\"` to start a JSON string"
        );
        *pos += 1;
        let mut s = String::new();
        loop {
            match chars.get(*pos) {
                Some('"') => {
                    *pos += 1;
                    break;
                }
                Some('\\') => {
                    *pos += 1;
                    match chars.get(*pos) {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('r') => s.push('\r'),
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('/') => s.push('/'),
                        Some('u') => {
                            let hex: String = chars[*pos + 1..*pos + 5].iter().collect();
                            let code = u32::from_str_radix(&hex, 16).expect("valid \\u escape");
                            s.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                            *pos += 4;
                        }
                        other => panic!("unsupported JSON escape: {other:?}"),
                    }
                    *pos += 1;
                }
                Some(&c) => {
                    s.push(c);
                    *pos += 1;
                }
                None => panic!("unterminated JSON string"),
            }
        }
        s
    }

    fn parse_number(chars: &[char], pos: &mut usize) -> Value {
        let start = *pos;
        if chars.get(*pos) == Some(&'-') {
            *pos += 1;
        }
        while matches!(chars.get(*pos), Some(c) if c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
        {
            *pos += 1;
        }
        let text: String = chars[start..*pos].iter().collect();
        Value::Number(
            text.parse()
                .unwrap_or_else(|_| panic!("invalid JSON number `{text}`")),
        )
    }
}
