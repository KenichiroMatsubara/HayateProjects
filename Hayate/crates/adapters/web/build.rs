use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let yaml_path = PathBuf::from(&manifest_dir).join("../../../proto/protocol.yaml");

    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml = fs::read_to_string(&yaml_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", yaml_path.display(), e));

    let proto = parse_yaml(&yaml);
    let code = generate(&proto);

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(out_dir).join("protocol.rs");
    fs::write(&out_path, code).unwrap();
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Proto {
    types: Vec<TypeDef>,
    enums: Vec<EnumDef>,
    opcodes: Vec<Entry>,
    style_tags: Vec<Entry>,
    event_kinds: Vec<Entry>,
    element_kinds: Vec<SimpleEntry>,
    unset_kinds: Vec<SimpleEntry>,
    modifier_keys: Vec<SimpleEntry>,
}

#[derive(Default)]
struct TypeDef {
    name: String,
    raw_slots: usize,
    fields: Vec<Param>,
}

#[derive(Default)]
struct EnumDef {
    name: String,
    string_values: bool,
    values: Vec<EnumValue>,
}

#[derive(Default, Clone)]
struct EnumValue {
    name: String,
    value: String, // numeric string or quoted string
}

#[derive(Default)]
struct Entry {
    name: String,
    value: u32,
    variable_length: bool,
    params: Vec<Param>,
}

#[derive(Default, Clone)]
struct Param {
    name: String,
    typ: String,
    count: usize, // 0 means 1 (no count attribute)
}

#[derive(Default)]
struct SimpleEntry {
    name: String,
    value: String,
}

// ---------------------------------------------------------------------------
// YAML parser
// ---------------------------------------------------------------------------

fn parse_yaml(yaml: &str) -> Proto {
    let mut proto = Proto::default();

    // section names
    const SEC_TYPES: &str = "types";
    const SEC_ENUMS: &str = "enums";
    const SEC_OPCODES: &str = "opcodes";
    const SEC_STYLE_TAGS: &str = "style_tags";
    const SEC_EVENT_KINDS: &str = "event_kinds";
    const SEC_ELEMENT_KINDS: &str = "element_kinds";
    const SEC_UNSET_KINDS: &str = "unset_kinds";
    const SEC_MODIFIER_KEYS: &str = "modifier_keys";

    let sections = [
        SEC_TYPES,
        SEC_ENUMS,
        SEC_OPCODES,
        SEC_STYLE_TAGS,
        SEC_EVENT_KINDS,
        SEC_ELEMENT_KINDS,
        SEC_UNSET_KINDS,
        SEC_MODIFIER_KEYS,
    ];

    let mut current_section: Option<&str> = None;

    // For types
    let mut cur_type: Option<TypeDef> = None;
    let mut cur_type_field: Option<Param> = None;

    // For enums
    let mut cur_enum: Option<EnumDef> = None;
    let mut cur_enum_val: Option<EnumValue> = None;

    // For opcodes / style_tags / event_kinds
    let mut cur_entry: Option<Entry> = None;
    let mut cur_param: Option<Param> = None;

    // For element_kinds / unset_kinds / modifier_keys
    let mut cur_simple: Option<SimpleEntry> = None;

    macro_rules! flush_type_field {
        () => {{
            if let Some(f) = cur_type_field.take() {
                if let Some(t) = cur_type.as_mut() {
                    t.fields.push(f);
                }
            }
        }};
    }
    macro_rules! flush_type {
        () => {{
            flush_type_field!();
            if let Some(t) = cur_type.take() {
                proto.types.push(t);
            }
        }};
    }

    macro_rules! flush_enum_val {
        () => {{
            if let Some(v) = cur_enum_val.take() {
                if let Some(e) = cur_enum.as_mut() {
                    e.values.push(v);
                }
            }
        }};
    }
    macro_rules! flush_enum {
        () => {{
            flush_enum_val!();
            if let Some(e) = cur_enum.take() {
                proto.enums.push(e);
            }
        }};
    }

    macro_rules! flush_param {
        () => {{
            if let Some(p) = cur_param.take() {
                if let Some(e) = cur_entry.as_mut() {
                    e.params.push(p);
                }
            }
        }};
    }
    macro_rules! flush_entry {
        () => {{
            flush_param!();
            if let Some(e) = cur_entry.take() {
                match current_section {
                    Some(SEC_OPCODES) => proto.opcodes.push(e),
                    Some(SEC_STYLE_TAGS) => proto.style_tags.push(e),
                    Some(SEC_EVENT_KINDS) => proto.event_kinds.push(e),
                    _ => {}
                }
            }
        }};
    }

    macro_rules! flush_simple {
        () => {{
            if let Some(s) = cur_simple.take() {
                match current_section {
                    Some(SEC_ELEMENT_KINDS) => proto.element_kinds.push(s),
                    Some(SEC_UNSET_KINDS) => proto.unset_kinds.push(s),
                    Some(SEC_MODIFIER_KEYS) => proto.modifier_keys.push(s),
                    _ => {}
                }
            }
        }};
    }

    for raw_line in yaml.lines() {
        // skip comments and empty lines
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // count leading spaces
        let indent = raw_line.len() - raw_line.trim_start().len();

        // section header: no indent, ends with ':'
        if indent == 0 {
            // Flush whatever was in progress
            match current_section {
                Some(SEC_TYPES) => flush_type!(),
                Some(SEC_ENUMS) => flush_enum!(),
                Some(SEC_OPCODES) | Some(SEC_STYLE_TAGS) | Some(SEC_EVENT_KINDS) => {
                    flush_entry!()
                }
                Some(SEC_ELEMENT_KINDS) | Some(SEC_UNSET_KINDS) | Some(SEC_MODIFIER_KEYS) => {
                    flush_simple!()
                }
                _ => {}
            }

            if let Some(sec_name) = trimmed.strip_suffix(':') {
                if sections.contains(&sec_name) {
                    current_section = Some(
                        sections
                            .iter()
                            .copied()
                            .find(|&s| s == sec_name)
                            .unwrap(),
                    );
                } else {
                    current_section = None;
                }
            }
            continue;
        }

        let sec = match current_section {
            Some(s) => s,
            None => continue,
        };

        // parse key: value from trimmed
        let (key, val) = if let Some(pos) = trimmed.find(':') {
            let k = trimmed[..pos].trim();
            let v = trimmed[pos + 1..].trim();
            (k, v)
        } else {
            continue;
        };

        // Remove surrounding quotes from value
        let val = val.trim_matches('"');

        match sec {
            SEC_TYPES => {
                if indent == 2 && key == "- name" {
                    flush_type!();
                    cur_type = Some(TypeDef {
                        name: val.to_string(),
                        ..Default::default()
                    });
                } else if indent == 4 && key == "raw_slots" {
                    if let Some(t) = cur_type.as_mut() {
                        t.raw_slots = val.parse().unwrap_or(0);
                    }
                } else if indent == 6 && key == "- name" {
                    flush_type_field!();
                    cur_type_field = Some(Param {
                        name: val.to_string(),
                        ..Default::default()
                    });
                } else if indent == 8 && key == "type" {
                    if let Some(f) = cur_type_field.as_mut() {
                        f.typ = val.to_string();
                    }
                }
            }

            SEC_ENUMS => {
                if indent == 2 && key == "- name" {
                    flush_enum!();
                    cur_enum = Some(EnumDef {
                        name: val.to_string(),
                        ..Default::default()
                    });
                } else if indent == 4 && key == "string_values" {
                    if let Some(e) = cur_enum.as_mut() {
                        e.string_values = val == "true";
                    }
                } else if indent == 6 && key == "- name" {
                    flush_enum_val!();
                    cur_enum_val = Some(EnumValue {
                        name: val.to_string(),
                        ..Default::default()
                    });
                } else if indent == 8 && key == "value" {
                    if let Some(v) = cur_enum_val.as_mut() {
                        v.value = val.to_string();
                    }
                }
            }

            SEC_OPCODES | SEC_STYLE_TAGS | SEC_EVENT_KINDS => {
                if indent == 2 && key == "- name" {
                    flush_entry!();
                    cur_entry = Some(Entry {
                        name: val.to_string(),
                        ..Default::default()
                    });
                } else if indent == 4 && key == "value" {
                    if let Some(e) = cur_entry.as_mut() {
                        e.value = val.parse().unwrap_or(0);
                    }
                } else if indent == 4 && key == "variable_length" {
                    if let Some(e) = cur_entry.as_mut() {
                        e.variable_length = val == "true";
                    }
                } else if indent == 6 && key == "- name" {
                    flush_param!();
                    cur_param = Some(Param {
                        name: val.to_string(),
                        ..Default::default()
                    });
                } else if indent == 8 && key == "type" {
                    if let Some(p) = cur_param.as_mut() {
                        p.typ = val.to_string();
                    }
                } else if indent == 8 && key == "count" {
                    if let Some(p) = cur_param.as_mut() {
                        p.count = val.parse().unwrap_or(0);
                    }
                }
            }

            SEC_ELEMENT_KINDS | SEC_UNSET_KINDS | SEC_MODIFIER_KEYS => {
                if indent == 2 && key == "- name" {
                    flush_simple!();
                    cur_simple = Some(SimpleEntry {
                        name: val.to_string(),
                        ..Default::default()
                    });
                } else if indent == 4 && key == "value" {
                    if let Some(s) = cur_simple.as_mut() {
                        s.value = val.to_string();
                    }
                }
            }

            _ => {}
        }
    }

    // flush last items
    match current_section {
        Some(SEC_TYPES) => flush_type!(),
        Some(SEC_ENUMS) => flush_enum!(),
        Some(SEC_OPCODES) | Some(SEC_STYLE_TAGS) | Some(SEC_EVENT_KINDS) => flush_entry!(),
        Some(SEC_ELEMENT_KINDS) | Some(SEC_UNSET_KINDS) | Some(SEC_MODIFIER_KEYS) => {
            flush_simple!()
        }
        _ => {}
    }

    proto
}

// ---------------------------------------------------------------------------
// Code generator
// ---------------------------------------------------------------------------

fn param_slots(p: &Param) -> usize {
    let count = if p.count == 0 { 1 } else { p.count };
    count
}

fn param_rust_type(typ: &str, count: usize) -> String {
    let base = match typ {
        "element_id" => "u64",
        "u32" => "u32",
        "f32" => "f32",
        "f64" => "f64",
        "bool" => "bool",
        "usize" => "usize",
        "string" => "String",
        other => other, // enums etc. – not used in Op/StyleTag
    };
    if count > 1 {
        format!("[{}; {}]", base, count)
    } else {
        base.to_string()
    }
}

fn generate(proto: &Proto) -> String {
    let mut out = String::new();

    out.push_str("// AUTO-GENERATED by build.rs — do not edit\n");
    out.push_str("// Source: proto/protocol.yaml\n\n");

    // OP_* constants
    out.push_str("// Opcode constants\n");
    for op in &proto.opcodes {
        out.push_str(&format!(
            "pub const OP_{}: u32 = {};\n",
            op.name, op.value
        ));
    }
    out.push('\n');

    // OP_SLOTS
    out.push_str("// Payload slot counts per opcode (op discriminant excluded)\n");
    out.push_str("pub const OP_SLOTS: &[usize] = &[\n");
    for op in &proto.opcodes {
        let slots: usize = op.params.iter().map(param_slots).sum();
        out.push_str(&format!(
            "    {}, // {}\n",
            slots, op.name
        ));
    }
    out.push_str("];\n\n");

    // TAG_* constants
    out.push_str("// Style tag constants\n");
    for tag in &proto.style_tags {
        out.push_str(&format!(
            "pub const TAG_{}: u32 = {};\n",
            tag.name, tag.value
        ));
    }
    out.push('\n');

    // EVENT_KIND_* constants (f64)
    out.push_str("// Event kind constants\n");
    for ev in &proto.event_kinds {
        out.push_str(&format!(
            "pub const EVENT_KIND_{}: f64 = {}.0;\n",
            ev.name.to_uppercase(),
            ev.value
        ));
    }
    out.push('\n');

    // ELEMENT_KIND_* constants
    out.push_str("// Element kind constants\n");
    for ek in &proto.element_kinds {
        out.push_str(&format!(
            "pub const ELEMENT_KIND_{}: u32 = {};\n",
            ek.name.to_uppercase(),
            ek.value
        ));
    }
    out.push('\n');

    // UNSET_KIND_* constants
    out.push_str("// Unset kind constants\n");
    for uk in &proto.unset_kinds {
        out.push_str(&format!(
            "pub const UNSET_KIND_{}: u32 = {};\n",
            uk.name.to_uppercase(),
            uk.value
        ));
    }
    out.push('\n');

    // MODIFIER_* constants
    out.push_str("// Modifier key constants\n");
    for mk in &proto.modifier_keys {
        out.push_str(&format!(
            "pub const MODIFIER_{}: u32 = {};\n",
            mk.name.to_uppercase(),
            mk.value
        ));
    }
    out.push('\n');

    // Op enum
    out.push_str("// Op enum\n");
    out.push_str("#[derive(Debug, Clone)]\n");
    out.push_str("pub enum Op {\n");
    for op in &proto.opcodes {
        // Convert SNAKE_UPPER to PascalCase
        let variant = to_pascal(&op.name);
        if op.params.is_empty() {
            out.push_str(&format!("    {},\n", variant));
        } else {
            out.push_str(&format!("    {} {{\n", variant));
            for p in &op.params {
                let count = if p.count == 0 { 1 } else { p.count };
                let rust_ty = param_rust_type(&p.typ, count);
                out.push_str(&format!("        {}: {},\n", p.name, rust_ty));
            }
            out.push_str("    },\n");
        }
    }
    out.push_str("}\n\n");

    // parse_next_op
    out.push_str("pub fn parse_next_op(ops: &[f64], i: usize) -> Result<(Op, usize), &'static str> {\n");
    out.push_str("    if i >= ops.len() { return Err(\"unexpected end of ops\"); }\n");
    out.push_str("    let disc = ops[i] as u32;\n");
    out.push_str("    let i = i + 1;\n");
    out.push_str("    match disc {\n");
    for op in &proto.opcodes {
        let variant = to_pascal(&op.name);
        let total_slots: usize = op.params.iter().map(param_slots).sum();
        out.push_str(&format!("        {} => {{\n", op.value));
        out.push_str(&format!(
            "            if i + {} > ops.len() {{ return Err(\"op {} truncated\"); }}\n",
            total_slots, op.name
        ));
        let mut slot = 0usize;
        for p in &op.params {
            let count = if p.count == 0 { 1 } else { p.count };
            if count > 1 {
                out.push_str(&format!("            let mut {} = [0f64; {}];\n", p.name, count));
                for ci in 0..count {
                    out.push_str(&format!(
                        "            {}[{}] = ops[i + {}];\n",
                        p.name,
                        ci,
                        slot + ci
                    ));
                }
            } else {
                let cast = match p.typ.as_str() {
                    "element_id" => "ops[i + IDX] as u64",
                    "u32" => "ops[i + IDX] as u32",
                    "f32" => "ops[i + IDX] as f32",
                    "f64" => "ops[i + IDX]",
                    "bool" => "ops[i + IDX] != 0.0",
                    "usize" => "ops[i + IDX] as usize",
                    _ => "ops[i + IDX] as u64",
                };
                let cast = cast.replace("IDX", &slot.to_string());
                out.push_str(&format!("            let {} = {};\n", p.name, cast));
            }
            slot += count;
        }
        if op.params.is_empty() {
            out.push_str(&format!("            Ok((Op::{}, i))\n", variant));
        } else {
            let field_list: Vec<String> = op.params.iter().map(|p| p.name.clone()).collect();
            out.push_str(&format!(
                "            Ok((Op::{} {{ {} }}, i + {}))\n",
                variant,
                field_list.join(", "),
                total_slots
            ));
        }
        out.push_str("        }\n");
    }
    out.push_str("        _ => Err(\"unknown opcode\"),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // StyleTag enum
    out.push_str("// StyleTag enum\n");
    out.push_str("#[derive(Debug, Clone)]\n");
    out.push_str("pub enum StyleTag {\n");
    for tag in &proto.style_tags {
        let variant = to_pascal(&tag.name);
        if tag.params.is_empty() {
            out.push_str(&format!("    {},\n", variant));
        } else {
            // flatten compound types to raw f32
            let fields = flatten_tag_params(&tag.params, proto);
            if fields.is_empty() {
                out.push_str(&format!("    {},\n", variant));
            } else {
                out.push_str(&format!("    {} {{\n", variant));
                for (fname, ftype) in &fields {
                    out.push_str(&format!("        {}: {},\n", fname, ftype));
                }
                out.push_str("    },\n");
            }
        }
    }
    out.push_str("}\n\n");

    // parse_next_style_tag
    out.push_str("pub fn parse_next_style_tag(packed: &[f32], i: usize) -> Result<(StyleTag, usize), &'static str> {\n");
    out.push_str("    if i >= packed.len() { return Err(\"unexpected end of style data\"); }\n");
    out.push_str("    let tag = packed[i] as u32;\n");
    out.push_str("    let i = i + 1;\n");
    out.push_str("    match tag {\n");
    for tag in &proto.style_tags {
        let variant = to_pascal(&tag.name);
        let fields = flatten_tag_params(&tag.params, proto);
        let slot_count = fields.len();
        out.push_str(&format!("        {} => {{\n", tag.value));
        if tag.variable_length {
            // string: first slot is byte length, then bytes packed as f32
            out.push_str("            if i >= packed.len() { return Err(\"style tag string truncated\"); }\n");
            out.push_str("            let byte_len = packed[i] as usize;\n");
            out.push_str("            let slot_len = (byte_len + 3) / 4;\n");
            out.push_str("            if i + 1 + slot_len > packed.len() { return Err(\"style tag string data truncated\"); }\n");
            out.push_str("            let bytes_start = i + 1;\n");
            out.push_str("            let mut bytes = Vec::with_capacity(byte_len);\n");
            out.push_str("            for si in 0..slot_len {\n");
            out.push_str("                let word = packed[bytes_start + si].to_bits();\n");
            out.push_str("                let remaining = byte_len - bytes.len();\n");
            out.push_str("                let take = remaining.min(4);\n");
            out.push_str("                for bi in 0..take {\n");
            out.push_str("                    bytes.push(((word >> (bi * 8)) & 0xFF) as u8);\n");
            out.push_str("                }\n");
            out.push_str("            }\n");
            out.push_str("            let family = String::from_utf8(bytes).map_err(|_| \"invalid utf8 in font family\")?;\n");
            out.push_str(&format!(
                "            Ok((StyleTag::{} {{ family }}, i + 1 + slot_len))\n",
                variant
            ));
        } else if fields.is_empty() {
            out.push_str(&format!("            Ok((StyleTag::{}, i))\n", variant));
        } else {
            out.push_str(&format!(
                "            if i + {} > packed.len() {{ return Err(\"style tag {} truncated\"); }}\n",
                slot_count, tag.name
            ));
            let mut si = 0usize;
            for (fname, _ftype) in &fields {
                out.push_str(&format!(
                    "            let {} = packed[i + {}];\n",
                    fname, si
                ));
                si += 1;
            }
            let field_list: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
            out.push_str(&format!(
                "            Ok((StyleTag::{} {{ {} }}, i + {}))\n",
                variant,
                field_list.join(", "),
                slot_count
            ));
        }
        out.push_str("        }\n");
    }
    out.push_str("        _ => Err(\"unknown style tag\"),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // encode_event
    out.push_str("pub fn encode_event(ev: &hayate_core::Event) -> js_sys::Array {\n");
    out.push_str("    use wasm_bindgen::JsValue;\n");
    out.push_str("    let sub = js_sys::Array::new();\n");
    out.push_str("    match ev {\n");

    out.push_str("        hayate_core::Event::Click { target_id, x, y } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(0.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*x as f64));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*y as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::Focus(target_id) => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(1.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::Blur(target_id) => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(2.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::TextInput { target_id, text } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(3.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("            sub.push(&JsValue::from_str(text));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::CompositionStart { target_id, text } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(4.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("            sub.push(&JsValue::from_str(text));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::CompositionUpdate { target_id, text } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(5.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("            sub.push(&JsValue::from_str(text));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::CompositionEnd { target_id, text } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(6.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("            sub.push(&JsValue::from_str(text));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::Scroll { target_id, delta_x, delta_y } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(7.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*delta_x as f64));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*delta_y as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::Resize { width, height } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(8.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*width as f64));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*height as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::ActiveEnd { target_id } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(9.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::HoverEnter { target_id } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(10.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::HoverLeave { target_id } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(11.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::KeyDown { target_id, key, modifiers } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(12.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("            sub.push(&JsValue::from_str(key));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*modifiers as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::ActiveStart { target_id } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(13.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(target_id.to_u64() as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::PointerMove { x, y } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(14.0));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*x as f64));\n");
    out.push_str("            sub.push(&JsValue::from_f64(*y as f64));\n");
    out.push_str("        }\n");

    out.push_str("        hayate_core::Event::FetchFont { family } => {\n");
    out.push_str("            sub.push(&JsValue::from_f64(15.0));\n");
    out.push_str("            sub.push(&JsValue::from_str(family));\n");
    out.push_str("        }\n");

    out.push_str("    }\n");
    out.push_str("    sub\n");
    out.push_str("}\n\n");

    // encode_events
    out.push_str("pub fn encode_events(events: &[hayate_core::Event]) -> js_sys::Array {\n");
    out.push_str("    let result = js_sys::Array::new();\n");
    out.push_str("    for ev in events {\n");
    out.push_str("        result.push(&encode_event(ev));\n");
    out.push_str("    }\n");
    out.push_str("    result\n");
    out.push_str("}\n");

    out
}

/// Flatten style tag params: expand compound types (color, dimension) into raw f32 fields.
fn flatten_tag_params(params: &[Param], proto: &Proto) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for p in params {
        // Check if this param type matches a known compound type
        if let Some(td) = proto.types.iter().find(|t| t.name == p.typ) {
            // Expand fields
            for f in &td.fields {
                let fname = format!("{}_{}", p.name, f.name);
                result.push((fname, "f32".to_string()));
            }
        } else if p.typ == "string" {
            result.push((p.name.clone(), "String".to_string()));
        } else {
            // f32, display enums, etc. — all stored as f32
            result.push((p.name.clone(), "f32".to_string()));
        }
    }
    result
}

/// Convert SCREAMING_SNAKE_CASE to PascalCase.
fn to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
            }
        })
        .collect()
}
