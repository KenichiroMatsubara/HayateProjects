use crate::{DomCss, Entry, Param, Proto};

/// Closed vocabulary for style-tag wire/codec/DOM codegen (generator-internal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueType {
    Color,
    Dimension,
    Scalar,
    /// Unsigned integer carried on one f32 wire slot (e.g. `max-lines`).
    U32,
    Enum(&'static str),
    DimensionList,
    FontFamily,
    ZIndex,
}

impl ValueType {
    /// Derive value type from spec fields only (`encodeFrom` + param type + `variable_length`).
    pub(crate) fn classify(tag: &Entry) -> Self {
        let encode_from = tag
            .encode_from
            .as_deref()
            .unwrap_or_else(|| panic!("style tag {} missing encodeFrom", tag.name));
        let primary_param = tag.params.first().map(|p| p.typ.as_str());

        match encode_from {
            "css-color" => ValueType::Color,
            "dimension" => ValueType::Dimension,
            "f32" => ValueType::Scalar,
            "u32" => ValueType::U32,
            "font-family" => {
                assert!(
                    tag.variable_length,
                    "font-family tag {} must set variable_length",
                    tag.name
                );
                assert_eq!(
                    primary_param,
                    Some("string"),
                    "font-family tag {} requires string param",
                    tag.name
                );
                ValueType::FontFamily
            }
            "dimension-list" => {
                assert!(
                    tag.variable_length,
                    "dimension-list tag {} must set variable_length",
                    tag.name
                );
                ValueType::DimensionList
            }
            "z-index" => {
                assert_eq!(
                    primary_param,
                    Some("i32"),
                    "z-index tag {} requires i32 param",
                    tag.name
                );
                ValueType::ZIndex
            }
            s if s.starts_with("enum:") => {
                let kind = &s["enum:".len()..];
                ValueType::Enum(enum_kind_to_static(kind))
            }
            other => panic!("unknown encodeFrom {other} for tag {}", tag.name),
        }
    }

    pub(crate) fn is_dimension_list(self) -> bool {
        matches!(self, ValueType::DimensionList)
    }

    pub(crate) fn encode_binding(self) -> &'static str {
        match self {
            ValueType::Color => "(c)",
            ValueType::Dimension => "(d)",
            ValueType::Scalar | ValueType::U32 | ValueType::Enum(_) => "(v)",
            ValueType::DimensionList => "(tracks)",
            ValueType::FontFamily => "(f)",
            ValueType::ZIndex => "(z)",
        }
    }

    pub(crate) fn encode_body(self) -> String {
        match self {
            ValueType::Color => "                let arr = c.to_array_f32();\n\
                buf.extend_from_slice(&arr);\n"
                .to_string(),
            ValueType::Dimension => "                buf.push(d.value);\n\
                buf.push(encode_dim_unit(d.unit));\n"
                .to_string(),
            ValueType::Scalar => "                buf.push(*v);\n".to_string(),
            ValueType::U32 => "                buf.push(*v as f32);\n".to_string(),
            ValueType::Enum(kind) => format!("                buf.push(encode_{kind}(*v));\n"),
            ValueType::DimensionList => [
                "                buf.push(tracks.len() as f32);",
                "                for d in tracks.iter().copied() {",
                "                    buf.push(d.value);",
                "                    buf.push(encode_dim_unit(d.unit));",
                "                }",
                "",
            ]
            .join("\n"),
            ValueType::FontFamily => "                let bytes = f.as_bytes();\n\
                buf.push(bytes.len() as f32);\n\
                for byte in bytes {\n\
                    buf.push(*byte as f32);\n\
                }\n"
                .to_string(),
            ValueType::ZIndex => "                buf.push(*z as f32);\n".to_string(),
        }
    }

    pub(crate) fn decode_to_prop_expr(self, params: &[Param], _proto: &Proto) -> String {
        match self {
            ValueType::Color => {
                let prefix = color_param_prefix(params);
                format!("codec_color({prefix}_r, {prefix}_g, {prefix}_b, {prefix}_a)")
            }
            ValueType::Dimension => {
                let name = &params[0].name;
                format!("codec_dim({name}_value, {name}_unit)")
            }
            ValueType::Scalar => "value".to_string(),
            ValueType::U32 => "value as u32".to_string(),
            ValueType::Enum(kind) => format!("codec_{kind}(value)"),
            ValueType::DimensionList => {
                "tracks.into_iter().map(|(value, unit)| codec_dim(value, unit)).collect()"
                    .to_string()
            }
            ValueType::FontFamily => "family".to_string(),
            ValueType::ZIndex => "value as i32".to_string(),
        }
    }

    pub(crate) fn match_binding(self) -> (String, &'static str) {
        match self {
            ValueType::Color => ("(c)".to_string(), "c"),
            ValueType::Dimension => ("(d)".to_string(), "d"),
            ValueType::Scalar | ValueType::U32 | ValueType::Enum(_) => ("(v)".to_string(), "v"),
            ValueType::DimensionList => ("(ref tracks)".to_string(), "tracks"),
            ValueType::FontFamily => ("(ref f)".to_string(), "f"),
            ValueType::ZIndex => ("(z)".to_string(), "z"),
        }
    }

    pub(crate) fn css_collect_lines(self, dom: &DomCss, value_var: &str) -> String {
        let css_prop = &dom.property;
        let mut lines = Vec::new();

        match self {
            ValueType::Color => {
                lines.push(format!(
                    "out.push((\"{css_prop}\".into(), dom_css_hex({value_var})));"
                ));
            }
            ValueType::Dimension => {
                lines.push(format!(
                    "out.push((\"{css_prop}\".into(), dom_css_dim({value_var})));"
                ));
            }
            ValueType::Scalar => match dom.format.as_str() {
                "px" => {
                    if dom.extras.is_empty() {
                        lines.push(format!(
                            "out.push((\"{css_prop}\".into(), format!(\"{{}}px\", {value_var}.max(0.0))));"
                        ));
                    } else {
                        lines.push(format!("let w = {value_var}.max(0.0);"));
                        lines.push(format!(
                            "out.push((\"{css_prop}\".into(), format!(\"{{}}px\", w)));"
                        ));
                    }
                }
                "number" => {
                    let expr = scalar_number_expr(css_prop, value_var);
                    lines.push(format!(
                        "out.push((\"{css_prop}\".into(), format!(\"{{}}\", {expr})));"
                    ));
                }
                other => panic!("Scalar tag domCss.format must be px or number, got {other}"),
            },
            ValueType::U32 => {
                // `w` is consumed by the shared `extras` loop below (whenPositive/whenZero).
                lines.push(format!("let w = {value_var} as f32;"));
                lines.push(format!(
                    "out.push((\"{css_prop}\".into(), format!(\"{{}}\", {value_var})));"
                ));
            }
            ValueType::ZIndex => {
                lines.push(format!(
                    "out.push((\"{css_prop}\".into(), {value_var}.to_string()));"
                ));
            }
            ValueType::FontFamily => {
                lines.push(format!(
                    "out.push((\"{css_prop}\".into(), {value_var}.to_string()));"
                ));
            }
            ValueType::DimensionList => {
                lines.push(format!(
                    "let s = {value_var}\n\
                .iter()\n\
                .map(|d| dom_css_dim(*d))\n\
                .collect::<Vec<_>>()\n\
                .join(\" \");"
                ));
                lines.push(format!("out.push((\"{css_prop}\".into(), s));"));
            }
            ValueType::Enum(kind) => {
                lines.push(enum_css_collect(css_prop, value_var, kind));
            }
        }

        for extra in &dom.extras {
            lines.push(format!(
                "out.push((\"{}\".into(), if w > 0.0 {{ \"{}\".into() }} else {{ \"{}\".into() }}));",
                extra.property, extra.when_positive, extra.when_zero
            ));
        }

        lines.join("\n")
    }
}

fn color_param_prefix(params: &[Param]) -> String {
    params
        .iter()
        .find(|p| p.typ == "color")
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "color".to_string())
}

fn enum_kind_to_static(kind: &str) -> &'static str {
    match kind {
        "display" => "display",
        "flex_direction" => "flex_direction",
        "flex_wrap" => "flex_wrap",
        "align_items" => "align_items",
        "align_self" => "align_self",
        "align_content" => "align_content",
        "justify_content" => "justify_content",
        "font_style" => "font_style",
        "text_decoration" => "text_decoration",
        "border_style" => "border_style",
        "cursor" => "cursor",
        "overflow" => "overflow",
        "text_overflow" => "text_overflow",
        "position" => "position",
        other => panic!("unknown enum encodeFrom kind: {other}"),
    }
}

fn scalar_number_expr(css_prop: &str, value_var: &str) -> String {
    match css_prop {
        "opacity" => format!("{value_var}.clamp(0.0, 1.0)"),
        "font-weight" => format!("{value_var}.clamp(1.0, 1000.0)"),
        "flex-grow" => format!("{value_var}.max(0.0)"),
        _ => value_var.to_string(),
    }
}

fn enum_css_collect(css_prop: &str, value_var: &str, kind: &str) -> String {
    let arms = match kind {
        "display" => {
            "DisplayValue::Flex => \"flex\",\n\
            DisplayValue::Grid => \"grid\",\n\
            DisplayValue::Block => \"block\",\n\
            DisplayValue::None => \"none\","
        }
        "flex_direction" => {
            "FlexDirectionValue::Row => \"row\",\n\
            FlexDirectionValue::Column => \"column\",\n\
            FlexDirectionValue::RowReverse => \"row-reverse\",\n\
            FlexDirectionValue::ColumnReverse => \"column-reverse\","
        }
        "flex_wrap" => {
            "FlexWrapValue::Nowrap => \"nowrap\",\n\
            FlexWrapValue::Wrap => \"wrap\",\n\
            FlexWrapValue::WrapReverse => \"wrap-reverse\","
        }
        "align_items" => {
            "AlignValue::FlexStart => \"flex-start\",\n\
            AlignValue::FlexEnd => \"flex-end\",\n\
            AlignValue::Center => \"center\",\n\
            AlignValue::Stretch => \"stretch\",\n\
            AlignValue::Baseline => \"baseline\","
        }
        "align_self" => {
            "AlignSelfValue::Auto => \"auto\",\n\
            AlignSelfValue::FlexStart => \"flex-start\",\n\
            AlignSelfValue::FlexEnd => \"flex-end\",\n\
            AlignSelfValue::Center => \"center\",\n\
            AlignSelfValue::Stretch => \"stretch\",\n\
            AlignSelfValue::Baseline => \"baseline\","
        }
        "align_content" => {
            "AlignContentValue::FlexStart => \"flex-start\",\n\
            AlignContentValue::FlexEnd => \"flex-end\",\n\
            AlignContentValue::Center => \"center\",\n\
            AlignContentValue::Stretch => \"stretch\",\n\
            AlignContentValue::SpaceBetween => \"space-between\",\n\
            AlignContentValue::SpaceAround => \"space-around\",\n\
            AlignContentValue::SpaceEvenly => \"space-evenly\","
        }
        "justify_content" => {
            "JustifyValue::FlexStart => \"flex-start\",\n\
            JustifyValue::FlexEnd => \"flex-end\",\n\
            JustifyValue::Center => \"center\",\n\
            JustifyValue::SpaceBetween => \"space-between\",\n\
            JustifyValue::SpaceAround => \"space-around\",\n\
            JustifyValue::SpaceEvenly => \"space-evenly\","
        }
        "font_style" => {
            "FontStyleValue::Normal => \"normal\",\n\
            FontStyleValue::Italic => \"italic\",\n\
            FontStyleValue::Oblique => \"oblique\","
        }
        "text_decoration" => {
            "TextDecorationValue::None => \"none\",\n\
            TextDecorationValue::Underline => \"underline\",\n\
            TextDecorationValue::LineThrough => \"line-through\","
        }
        "border_style" => {
            "BorderStyleValue::None => \"none\",\n\
            BorderStyleValue::Solid => \"solid\",\n\
            BorderStyleValue::Dashed => \"dashed\","
        }
        "cursor" => {
            "CursorValue::Default => \"default\",\n\
            CursorValue::Pointer => \"pointer\",\n\
            CursorValue::Text => \"text\",\n\
            CursorValue::Crosshair => \"crosshair\",\n\
            CursorValue::NotAllowed => \"not-allowed\",\n\
            CursorValue::Grab => \"grab\",\n\
            CursorValue::Grabbing => \"grabbing\","
        }
        "overflow" => {
            "OverflowValue::Visible => \"visible\",\n\
            OverflowValue::Hidden => \"hidden\","
        }
        "text_overflow" => {
            "TextOverflowValue::Clip => \"clip\",\n\
            TextOverflowValue::Ellipsis => \"ellipsis\","
        }
        "position" => {
            "PositionValue::Relative => \"relative\",\n\
            PositionValue::Absolute => \"absolute\","
        }
        other => panic!("unknown enum domCss kind: {other}"),
    };
    format!(
        "let s = match {value_var} {{\n\
            {arms}\n\
            }};\n\
            out.push((\"{css_prop}\".into(), s.into()));"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tag(encode_from: &str, params: Vec<(&str, &str)>, variable_length: bool) -> Entry {
        Entry {
            encode_from: Some(encode_from.to_string()),
            variable_length,
            params: params
                .into_iter()
                .map(|(name, typ)| Param {
                    name: name.to_string(),
                    typ: typ.to_string(),
                    count: 0,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn classify_derives_color_from_encode_from() {
        let tag = sample_tag("css-color", vec![("c", "color")], false);
        assert_eq!(ValueType::classify(&tag), ValueType::Color);
    }

    #[test]
    fn classify_derives_dimension_from_encode_from() {
        let tag = sample_tag("dimension", vec![("d", "dimension")], false);
        assert_eq!(ValueType::classify(&tag), ValueType::Dimension);
    }

    #[test]
    fn classify_derives_scalar_from_encode_from() {
        let tag = sample_tag("f32", vec![("value", "f32")], false);
        assert_eq!(ValueType::classify(&tag), ValueType::Scalar);
    }

    #[test]
    fn classify_derives_enum_from_encode_from() {
        let tag = sample_tag("enum:display", vec![("value", "display")], false);
        assert_eq!(ValueType::classify(&tag), ValueType::Enum("display"));
    }

    #[test]
    fn classify_derives_font_family_from_encode_from_and_param_type() {
        let tag = sample_tag("font-family", vec![("family", "string")], true);
        assert_eq!(ValueType::classify(&tag), ValueType::FontFamily);
    }

    #[test]
    fn classify_derives_dimension_list_from_encode_from() {
        let tag = sample_tag("dimension-list", vec![("tracks", "dimension")], true);
        assert_eq!(ValueType::classify(&tag), ValueType::DimensionList);
    }

    #[test]
    fn classify_derives_z_index_from_encode_from_and_i32_param() {
        let tag = sample_tag("z-index", vec![("value", "i32")], false);
        assert_eq!(ValueType::classify(&tag), ValueType::ZIndex);
    }

    #[test]
    fn encode_binding_matches_style_prop_variant_shape() {
        assert_eq!(ValueType::Color.encode_binding(), "(c)");
        assert_eq!(ValueType::ZIndex.encode_binding(), "(z)");
        assert_eq!(ValueType::DimensionList.encode_binding(), "(tracks)");
    }
}
