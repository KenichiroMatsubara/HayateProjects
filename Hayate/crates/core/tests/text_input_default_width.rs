//! UA default width for `text-input` (issue #403, ADR-0109 root cause A).
//!
//! A `text-input` with no explicit `width` must carry a font-relative intrinsic
//! content width (the browser `<input size=20>` default) so it does not collapse
//! to padding-only width on the Canvas path. These tests drive the behavior
//! through the public document API (`render` + `element_layout_rect`), the same
//! path both Scene Renderers observe.

use hayate_core::{
    AlignValue, BorderStyleValue, Color, Dimension, DisplayValue, ElementId, ElementKind,
    ElementTree, FlexDirectionValue, StyleProp,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// Border + horizontal padding of `input_style()`: padding 12+12, border 1+1.
const INPUT_CHROME_PX: f32 = 26.0;

fn input_style() -> Vec<StyleProp> {
    // Mirrors theme.ts `inputStyle()` — NO width, NO flex-grow.
    vec![
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::PaddingLeft(Dimension::px(12.0)),
        StyleProp::PaddingRight(Dimension::px(12.0)),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(Color::new(0.85, 0.83, 0.78, 1.0)),
        StyleProp::FontSize(13.0),
    ]
}

struct Builder {
    tree: ElementTree,
    next: u64,
}

impl Builder {
    fn new() -> Self {
        let mut tree = ElementTree::new();
        tree.register_font("Inter", FONT.to_vec());
        Self { tree, next: 1 }
    }

    fn mk(&mut self, kind: ElementKind, styles: &[StyleProp]) -> ElementId {
        let id = self.tree.element_create(self.next, kind);
        self.next += 1;
        self.tree.element_set_style(id, styles);
        id
    }
}

/// Build the gallery `PopCard` container (column flex, `align-items: flex-start`)
/// holding a single `text-input`, render it, and return the input's border-box
/// width.
fn input_border_box_width(input_styles: &[StyleProp], placeholder: &str) -> f32 {
    let mut b = Builder::new();
    let root = b.mk(
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::DefaultFontFamily("Inter".to_string()),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    b.tree.set_root(root);
    b.tree.set_viewport(300.0, 400.0);

    let demo = b.mk(
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Padding(Dimension::px(14.0)),
        ],
    );
    b.tree.element_append_child(root, demo);

    let input = b.mk(ElementKind::TextInput, input_styles);
    b.tree.element_set_text(input, placeholder);
    b.tree.element_append_child(demo, input);

    b.tree.render(0.0);
    let (_x, _y, iw, _h) = b
        .tree
        .element_layout_rect(input)
        .expect("input must have layout geometry");
    iw
}

/// Content width of an `input_style()` field (border box minus its chrome).
fn input_content_width(placeholder: &str) -> f32 {
    input_border_box_width(&input_style(), placeholder) - INPUT_CHROME_PX
}

#[test]
fn width_unspecified_text_input_gets_font_relative_default_width() {
    // Without the UA default the input collapses to ~0 content width (placeholder
    // wraps 1 char/line). With it, ~20 chars at 13px sit on one line: well above 50px.
    let content_width = input_content_width("Type here");
    assert!(
        content_width > 50.0,
        "width-unspecified text-input must carry a non-trivial default width, got {content_width}"
    );
}

#[test]
fn default_width_scales_with_font_size() {
    // The UA default is N chars in the *current* font, so a larger font-size
    // widens the field proportionally (browser `<input>`, not a fixed px).
    let small = input_border_box_width(
        &[StyleProp::Height(Dimension::px(38.0)), StyleProp::FontSize(13.0)],
        "x",
    );
    let large = input_border_box_width(
        &[StyleProp::Height(Dimension::px(60.0)), StyleProp::FontSize(26.0)],
        "x",
    );
    // Doubling font-size should roughly double the default width.
    assert!(
        large > small * 1.7,
        "default width must follow font-size: 13px gave {small}, 26px gave {large}"
    );
}

#[test]
fn explicit_width_overrides_default() {
    // An explicit `width` must win over the UA default (Taffy intrinsic order:
    // explicit > element-kind default). The field is exactly the requested width.
    let mut styles = input_style();
    styles.push(StyleProp::Width(Dimension::px(80.0)));
    let border_box = input_border_box_width(&styles, "Type here");
    assert!(
        (border_box - 80.0).abs() < 0.5,
        "explicit width:80 must override the default, got border-box width {border_box}"
    );
}

#[test]
fn flex_grow_grows_past_default() {
    // The addform case: an input with `flex-grow:1` in a row must stretch to fill
    // the row, not stop at the ~20-char default (Taffy intrinsic order: grow wins).
    let mut b = Builder::new();
    let root = b.mk(
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::DefaultFontFamily("Inter".to_string()),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    b.tree.set_root(root);
    b.tree.set_viewport(300.0, 100.0);

    let mut styles = input_style();
    styles.push(StyleProp::FlexGrow(1.0));
    let input = b.mk(ElementKind::TextInput, &styles);
    b.tree.element_set_text(input, "x");
    b.tree.element_append_child(root, input);

    b.tree.render(0.0);
    let (_x, _y, iw, _h) = b
        .tree
        .element_layout_rect(input)
        .expect("input must have layout geometry");
    // Sole flex-grow child of a 300px row fills it (minus nothing): ~300px ≫ 20ch.
    assert!(
        iw > 280.0,
        "flex-grow:1 input must fill the row past the default width, got {iw}"
    );
}

#[test]
fn default_width_is_independent_of_text_content() {
    // The browser `<input size>` default fixes the field width and scrolls its
    // value; it does not grow to fit. A short and a long placeholder must yield
    // the same default width.
    let short = input_content_width("a");
    let long = input_content_width("a very long placeholder that far exceeds twenty characters");
    assert!(
        (short - long).abs() < 0.5,
        "default width must not depend on text content: short={short}, long={long}"
    );
}
