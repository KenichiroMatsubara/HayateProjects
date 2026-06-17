use hayate_core::{
    AlignContentValue, AlignSelfValue, AlignValue, BorderStyleValue, Color, Dimension, DisplayValue,
    ElementId, ElementKind, ElementTree, FlexDirectionValue, FlexWrapValue, JustifyValue,
    OverflowValue, Shadow, StyleProp, TextDecorationValue,
};

use crate::pixel::{assert_channel_min, assert_channel_max, assert_clear, assert_not_clear, pixel};
use crate::pixel::CANVAS_W;

const VW: f32 = 100.0;
const VH: f32 = 100.0;

static NOTO_SANS_JP_BYTES: &[u8] = include_bytes!("../../../core/assets/fonts/NotoSansJP.ttf");

/// A small COLRv1 + CPAL test font (provenance in `assets/PROVENANCE.md`). Used
/// to prove the Vello backend paints colour glyphs (issue #332).
static COLR_TEST_BYTES: &[u8] = include_bytes!("../assets/colr_test_glyphs.ttf");

/// Family the COLR test font is registered under, and a PUA codepoint it maps
/// to a gradient glyph painted from a rainbow palette (`U+F0100`).
pub const COLOR_GLYPH_FAMILY: &str = "Colr Test";
pub const COLOR_GLYPH_CODEPOINT: char = '\u{F0100}';

fn register_bundled_font(tree: &mut ElementTree) {
    tree.register_font("Noto Sans", NOTO_SANS_JP_BYTES.to_vec());
}

/// A tree rendering a single COLRv1 glyph from [`COLR_TEST_BYTES`], large enough
/// to fill the canvas. When a backend honours COLR (Vello), the painted pixels
/// span several hues; a monochrome painter would yield a single ink colour.
pub fn color_glyph_tree() -> ElementTree {
    let mut tree = ElementTree::new();
    tree.register_font(COLOR_GLYPH_FAMILY, COLR_TEST_BYTES.to_vec());
    let root = root_view(&mut tree, 70);
    let text = child_text(&mut tree, 71);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, text);
    tree.element_set_style(
        text,
        &[
            StyleProp::FontFamily(COLOR_GLYPH_FAMILY.to_string()),
            StyleProp::FontSize(80.0),
        ],
    );
    tree.element_set_text(text, &COLOR_GLYPH_CODEPOINT.to_string());
    tree
}

fn viewport(tree: &mut ElementTree) {
    tree.set_viewport(VW, VH);
}

fn root_view(tree: &mut ElementTree, id: u64) -> ElementId {
    let root = tree.element_create(id, ElementKind::View);
    tree.set_root(root);
    viewport(tree);
    root
}

fn child_view(tree: &mut ElementTree, id: u64) -> ElementId {
    tree.element_create(id, ElementKind::View)
}

fn child_text(tree: &mut ElementTree, id: u64) -> ElementId {
    tree.element_create(id, ElementKind::Text)
}

pub struct CssPixelCase {
    /// `style_tags.json` / catalog `cssProperty` name.
    pub css_property: &'static str,
    pub build: fn() -> ElementTree,
    pub check: fn(&[u8]),
}

// ── visual ────────────────────────────────────────────────────────────────

fn build_background_color() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 1);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_background_color(data: &[u8]) {
    let px = pixel(data, CANVAS_W, 30, 30);
    assert_channel_min(px, 0, 200, "background-color center red");
    assert_channel_max(px, 1, 30, "background-color center red");
}

fn build_opacity() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 2);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::Opacity(0.4),
        ],
    );
    tree
}

fn check_opacity(data: &[u8]) {
    let px = pixel(data, CANVAS_W, 30, 30);
    // Opacity multiplies color alpha then composites on white clear → pink-ish fill.
    assert_channel_min(px, 0, 240, "opacity center red channel");
    assert_channel_min(px, 1, 120, "opacity center green from white blend");
    assert_channel_max(px, 1, 180, "opacity center green from white blend");
}

fn build_border_radius() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 3);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            StyleProp::BorderRadius(14.0),
        ],
    );
    tree
}

fn check_border_radius(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 2, 2), "border-radius outer corner clear");
    let center = pixel(data, CANVAS_W, 30, 30);
    assert_channel_min(center, 2, 200, "border-radius center blue");
}

fn build_border_width() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 4);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BorderWidth(6.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(Color::new(0.0, 0.0, 0.0, 1.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_border_width(data: &[u8]) {
    let edge = pixel(data, CANVAS_W, 30, 0);
    assert_channel_max(edge, 0, 30, "border-width top edge black");
    let center = pixel(data, CANVAS_W, 30, 30);
    assert_channel_min(center, 0, 200, "border-width center white");
}

fn build_border_color() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 5);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BorderWidth(4.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(Color::new(0.0, 0.5, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_border_color(data: &[u8]) {
    let edge = pixel(data, CANVAS_W, 30, 1);
    assert_channel_min(edge, 1, 100, "border-color green border");
    assert_channel_max(edge, 0, 30, "border-color green border");
}

fn build_box_shadow() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 70);
    // Opaque white box at (0,0)-(50,50) with a hard black drop shadow offset
    // down-right by 10px — the visible shadow is the L outside the box.
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 10.0,
                offset_y: 10.0,
                blur: 0.0,
                spread: 0.0,
                color: Color::new(0.0, 0.0, 0.0, 1.0),
                inset: false,
            }]),
        ],
    );
    tree
}

fn check_box_shadow(data: &[u8]) {
    // Box interior stays white (shadow is painted behind the opaque box).
    let center = pixel(data, CANVAS_W, 25, 25);
    assert_channel_min(center, 0, 200, "box-shadow box center white");
    // The offset shadow is visible just right of and below the box.
    let shadow = pixel(data, CANVAS_W, 55, 30);
    assert_channel_max(shadow, 0, 60, "box-shadow drop region dark");
    // Far from box and shadow, the canvas is clear.
    assert_clear(pixel(data, CANVAS_W, 90, 90), "box-shadow far corner clear");
}

fn build_box_shadow_inset() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 71);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur: 0.0,
                spread: 12.0,
                color: Color::new(0.0, 0.0, 0.0, 1.0),
                inset: true,
            }]),
        ],
    );
    tree
}

fn check_box_shadow_inset(data: &[u8]) {
    // Inner edge band is darkened over the white background…
    let edge = pixel(data, CANVAS_W, 3, 30);
    assert_channel_max(edge, 0, 180, "box-shadow inset edge darkened");
    // …while the centre stays light, and the shadow never escapes the box.
    let center = pixel(data, CANVAS_W, 30, 30);
    assert_channel_min(center, 0, 200, "box-shadow inset center light");
    assert_clear(pixel(data, CANVAS_W, 80, 30), "box-shadow inset stays inside box");
}

fn build_border_style() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 6);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BorderWidth(6.0),
            StyleProp::BorderStyle(BorderStyleValue::Dashed),
            StyleProp::BorderColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_border_style(data: &[u8]) {
    // A dashed top edge has both blue dashes and white gaps across its run,
    // which distinguishes it from a solid border (no gaps) and none (no dashes).
    let mut dashes = 0;
    let mut gaps = 0;
    for x in 2..58 {
        let px = pixel(data, CANVAS_W, x, 2);
        if px[2] > 150 && px[0] < 80 {
            dashes += 1;
        } else if px[0] > 200 && px[1] > 200 && px[2] > 200 {
            gaps += 1;
        }
    }
    assert!(dashes > 0, "border-style dashed paints blue dashes on the top edge");
    assert!(gaps > 0, "border-style dashed leaves white gaps between dashes");
}

// ── border / focus-ring rasterisation (issue #337) ──────────────────────────

/// A keyboard-focused text input with an opaque fill. The native focus ring
/// (`:focus-visible`, #335) is a `RoundedRing` painted *on top* of the box, so
/// it must hollow only its band — never erase the content it overlays. tiny-skia
/// previously cleared the ring's interior, punching the input to transparent.
fn build_focus_ring_over_fill() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 600);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    let input = tree.element_create(601, ElementKind::TextInput);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::BorderRadius(8.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, input);
    tree.element_focus(input); // keyboard/pointer focus → `:focus-visible` ring
    tree
}

fn check_focus_ring_over_fill(data: &[u8]) {
    // The focus ring must not erase the input it sits on: the interior stays the
    // opaque red fill (not the transparent hole tiny-skia's Clear used to punch).
    let center = pixel(data, CANVAS_W, 24, 20);
    assert_channel_min(center, 0, 200, "focus ring preserves the input fill (red)");
    assert_channel_max(center, 1, 60, "focus ring did not erase the input interior");
}

/// A 1px solid border on an opaque box at integer coordinates must land as an
/// independent opaque column — the literal acceptance probe for issue #337 (the
/// hairline must not be swallowed by the fill).
fn build_border_hairline_1px() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 610);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.6, 0.0, 1.0)),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_border_hairline_1px(data: &[u8]) {
    // Top row (y=0) is the 1px border: an opaque black column, not the fill.
    let edge = pixel(data, CANVAS_W, 30, 0);
    assert_channel_max(edge, 1, 70, "1px border top edge is black (independent column)");
    // One row inside is the green fill — the border did not bleed it away.
    let inside = pixel(data, CANVAS_W, 30, 3);
    assert_channel_min(inside, 1, 120, "fill just inside the 1px border is green");
}

fn build_overflow_hidden() -> ElementTree {
    // A solid child fully covers a rounded `overflow: hidden` parent. The
    // rounded clip must carve the child's square corner away (issue #206).
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 7);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BorderRadius(20.0),
            StyleProp::Overflow(OverflowValue::Hidden),
        ],
    );
    let child = child_view(&mut tree, 70);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, child);
    tree
}

fn check_overflow_hidden(data: &[u8]) {
    assert_clear(
        pixel(data, CANVAS_W, 2, 2),
        "overflow:hidden rounded corner clips the child",
    );
    let center = pixel(data, CANVAS_W, 30, 30);
    assert_channel_min(center, 0, 200, "overflow:hidden center shows the red child");
}

// ── sizing ────────────────────────────────────────────────────────────────

fn build_width() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 10);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_width(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 20, 20), "width inside box");
    assert_clear(pixel(data, CANVAS_W, 60, 20), "width outside box");
}

fn build_height() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 11);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_height(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 20, 20), "height inside box");
    assert_clear(pixel(data, CANVAS_W, 20, 60), "height outside box");
}

fn build_min_width() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 12);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(20.0)),
            StyleProp::MinWidth(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.5, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_min_width(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 40, 15), "min-width expanded box");
    assert_clear(pixel(data, CANVAS_W, 55, 15), "min-width beyond min");
}

fn build_min_height() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 13);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(10.0)),
            StyleProp::MinHeight(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.5, 0.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_min_height(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 15, 40), "min-height expanded box");
    assert_clear(pixel(data, CANVAS_W, 15, 55), "min-height beyond min");
}

fn build_max_width() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 14);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(80.0)),
            StyleProp::MaxWidth(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_max_width(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 35, 15), "max-width inside cap");
    assert_clear(pixel(data, CANVAS_W, 50, 15), "max-width beyond cap");
}

fn build_max_height() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 15);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::MaxHeight(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_max_height(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 15, 35), "max-height inside cap");
    assert_clear(pixel(data, CANVAS_W, 15, 50), "max-height beyond cap");
}

// ── layout ────────────────────────────────────────────────────────────────

fn flex_row_root(tree: &mut ElementTree, id: u64) -> ElementId {
    let root = root_view(tree, id);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    root
}

fn build_display_flex() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = flex_row_root(&mut tree, 20);
    let child = child_view(&mut tree, 21);
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_display_flex(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 10, 10), "display:flex child visible");
}

fn build_display_none() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = flex_row_root(&mut tree, 22);
    let child = child_view(&mut tree, 23);
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Display(DisplayValue::None),
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_display_none(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 10, 10), "display:none child hidden");
}

fn build_display_grid() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 24);
    let child = child_view(&mut tree, 25);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(35.0)),
            StyleProp::Height(Dimension::px(35.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_display_grid(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 10, 10), "display:grid child visible");
}

fn build_grid_template_columns_fr() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 50);
    let left = child_view(&mut tree, 51);
    let right = child_view(&mut tree, 52);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
            StyleProp::GridTemplateColumns(vec![Dimension::fr(1.0), Dimension::fr(1.0)]),
        ],
    );
    tree.element_append_child(root, left);
    tree.element_append_child(root, right);
    tree.element_set_style(
        left,
        &[
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style(
        right,
        &[
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_grid_template_columns_fr(data: &[u8]) {
    let left = pixel(data, CANVAS_W, 20, 25);
    let right = pixel(data, CANVAS_W, 75, 25);
    assert_channel_min(left, 0, 200, "grid 1fr left column red");
    assert_channel_max(left, 1, 30, "grid 1fr left column red");
    assert_channel_min(right, 1, 200, "grid 1fr right column green");
    assert_channel_max(right, 0, 30, "grid 1fr right column green");
}

fn build_grid_template_columns_px() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 53);
    let left = child_view(&mut tree, 54);
    let right = child_view(&mut tree, 55);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
            StyleProp::GridTemplateColumns(vec![Dimension::px(35.0), Dimension::px(65.0)]),
        ],
    );
    tree.element_append_child(root, left);
    tree.element_append_child(root, right);
    tree.element_set_style(
        left,
        &[
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style(
        right,
        &[
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_grid_template_columns_px(data: &[u8]) {
    let left = pixel(data, CANVAS_W, 15, 25);
    let right = pixel(data, CANVAS_W, 70, 25);
    assert_channel_min(left, 0, 200, "grid px left column red");
    assert_channel_max(left, 2, 30, "grid px left column red");
    assert_channel_min(right, 2, 200, "grid px right column blue");
    assert_channel_max(right, 0, 30, "grid px right column blue");
}

fn build_flex_direction() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 26);
    let a = child_view(&mut tree, 27);
    let b = child_view(&mut tree, 28);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_set_style(root, &[StyleProp::Gap(Dimension::px(15.0))]);
    for child in [a, b] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(30.0)),
                StyleProp::Height(Dimension::px(20.0)),
                StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ],
        );
    }
    tree
}

fn check_flex_direction(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 10, 10), "flex-direction first child top");
    assert_not_clear(pixel(data, CANVAS_W, 10, 48), "flex-direction second child below");
    assert_clear(pixel(data, CANVAS_W, 10, 32), "flex-direction gap between");
}

fn build_align_items() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 30);
    let child = child_view(&mut tree, 31);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_align_items(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 15, 5), "align-items top margin clear");
    assert_not_clear(pixel(data, CANVAS_W, 15, 35), "align-items centered child");
}

fn build_justify_content() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 32);
    let child = child_view(&mut tree, 33);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_justify_content(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 5, 15), "justify-content left margin clear");
    assert_not_clear(pixel(data, CANVAS_W, 50, 15), "justify-content centered child");
}

fn build_gap() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = flex_row_root(&mut tree, 34);
    for id in [35u64, 36] {
        let child = child_view(&mut tree, id);
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(20.0)),
                StyleProp::Height(Dimension::px(20.0)),
                StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ],
        );
    }
    tree.element_set_style(root, &[StyleProp::Gap(Dimension::px(20.0))]);
    tree
}

fn check_gap(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 5, 5), "gap first child");
    assert_clear(pixel(data, CANVAS_W, 28, 5), "gap between children");
    assert_not_clear(pixel(data, CANVAS_W, 45, 5), "gap second child");
}

fn padded_child_tree(padding: StyleProp) -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 40);
    let outer = child_view(&mut tree, 41);
    let inner = child_view(&mut tree, 42);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, outer);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BackgroundColor(Color::new(0.8, 0.8, 0.8, 1.0)),
            padding,
        ],
    );
    tree.element_append_child(outer, inner);
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(20.0)),
            StyleProp::Height(Dimension::px(20.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree
}

fn build_padding() -> ElementTree {
    padded_child_tree(StyleProp::Padding(Dimension::px(15.0)))
}

fn check_padding(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 20, 20), "padding inner child offset");
    assert_clear(pixel(data, CANVAS_W, 80, 80), "padding outside outer box");
}

fn build_padding_top() -> ElementTree {
    padded_child_tree(StyleProp::PaddingTop(Dimension::px(20.0)))
}

fn check_padding_top(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 18, 22), "padding-top child lowered");
    assert_clear(pixel(data, CANVAS_W, 80, 80), "padding-top outside outer box");
}

fn build_padding_right() -> ElementTree {
    padded_child_tree(StyleProp::PaddingRight(Dimension::px(25.0)))
}

fn check_padding_right(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 5, 18), "padding-right child left");
    assert_clear(pixel(data, CANVAS_W, 80, 80), "padding-right outside outer box");
}

fn build_padding_bottom() -> ElementTree {
    padded_child_tree(StyleProp::PaddingBottom(Dimension::px(20.0)))
}

fn check_padding_bottom(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 5, 5), "padding-bottom child top");
    assert_clear(pixel(data, CANVAS_W, 80, 80), "padding-bottom outside outer box");
}

fn build_padding_left() -> ElementTree {
    padded_child_tree(StyleProp::PaddingLeft(Dimension::px(20.0)))
}

fn check_padding_left(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 25, 18), "padding-left child shifted");
    assert_clear(pixel(data, CANVAS_W, 80, 80), "padding-left outside outer box");
}

fn margined_child_tree(margin: StyleProp) -> ElementTree {
    let mut tree = ElementTree::new();
    let root = flex_row_root(&mut tree, 50);
    let child = child_view(&mut tree, 51);
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(25.0)),
            StyleProp::Height(Dimension::px(25.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.5, 1.0, 1.0)),
            margin,
        ],
    );
    tree
}

fn build_margin() -> ElementTree {
    margined_child_tree(StyleProp::Margin(Dimension::px(15.0)))
}

fn check_margin(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 5, 5), "margin inset clear");
    assert_not_clear(pixel(data, CANVAS_W, 20, 20), "margin child inset");
}

fn build_margin_top() -> ElementTree {
    margined_child_tree(StyleProp::MarginTop(Dimension::px(20.0)))
}

fn check_margin_top(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 5, 5), "margin-top clear");
    assert_not_clear(pixel(data, CANVAS_W, 5, 25), "margin-top child down");
}

fn build_margin_right() -> ElementTree {
    margined_child_tree(StyleProp::MarginRight(Dimension::px(40.0)))
}

fn check_margin_right(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 5, 5), "margin-right child left");
}

fn build_margin_bottom() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 52);
    let child = child_view(&mut tree, 53);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(25.0)),
            StyleProp::Height(Dimension::px(25.0)),
            StyleProp::MarginBottom(Dimension::px(30.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.5, 1.0, 1.0)),
        ],
    );
    tree
}

fn check_margin_bottom(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 5, 5), "margin-bottom child top");
    assert_clear(pixel(data, CANVAS_W, 5, 40), "margin-bottom below child");
}

fn build_margin_left() -> ElementTree {
    margined_child_tree(StyleProp::MarginLeft(Dimension::px(25.0)))
}

fn check_margin_left(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 5, 5), "margin-left clear");
    assert_not_clear(pixel(data, CANVAS_W, 30, 5), "margin-left child right");
}

// ── text ──────────────────────────────────────────────────────────────────

pub fn text_tree(extra: &[StyleProp]) -> ElementTree {
    let mut tree = ElementTree::new();
    register_bundled_font(&mut tree);
    let root = root_view(&mut tree, 60);
    let text = child_text(&mut tree, 61);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, text);
    let mut styles = vec![StyleProp::FontSize(24.0)];
    styles.extend_from_slice(extra);
    tree.element_set_style(text, &styles);
    tree.element_set_text(text, "A");
    tree
}

fn build_font_size() -> ElementTree {
    text_tree(&[])
}

fn check_font_size(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 4, 20), "font-size text ink");
}

fn build_color() -> ElementTree {
    text_tree(&[StyleProp::Color(Color::new(1.0, 0.0, 0.0, 1.0))])
}

fn check_color(data: &[u8]) {
    let px = pixel(data, CANVAS_W, 4, 20);
    assert_channel_min(px, 0, 150, "color red glyph");
    assert_channel_max(px, 1, 80, "color red glyph");
}

fn build_font_family() -> ElementTree {
    text_tree(&[StyleProp::FontFamily("Noto Sans".to_string())])
}

fn check_font_family(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 4, 20), "font-family renders");
}

fn build_font_weight() -> ElementTree {
    text_tree(&[StyleProp::FontWeight(700.0)])
}

fn check_font_weight(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 4, 20), "font-weight bold renders");
}

fn build_text_decoration_underline() -> ElementTree {
    text_tree(&[
        StyleProp::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
        StyleProp::TextDecoration(TextDecorationValue::Underline),
    ])
}

fn check_text_decoration_underline(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 4, 20), "underline glyph body");
    // Underline sits below the alphabetic baseline (~y=27 for 24px "A"); not at strikethrough height.
    assert_clear(pixel(data, CANVAS_W, 4, 24), "underline not at strikethrough height");
    assert_not_clear(pixel(data, CANVAS_W, 4, 30), "underline decoration below baseline");
}

fn build_text_decoration_line_through() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 62);
    let text = child_text(&mut tree, 63);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, text);
    tree.element_set_style(
        text,
        &[
            StyleProp::FontSize(24.0),
            StyleProp::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
            StyleProp::TextDecoration(TextDecorationValue::LineThrough),
        ],
    );
    tree.element_set_text(text, "O");
    tree
}

fn check_text_decoration_line_through(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 8, 20), "line-through decoration ink");
    assert_clear(pixel(data, CANVAS_W, 8, 35), "line-through not at glyph bottom");
}

// ── stacking / flex ─────────────────────────────────────────────────────

fn build_z_index() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 70);
    let back = child_view(&mut tree, 71);
    let front = child_view(&mut tree, 72);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, back);
    tree.element_append_child(root, front);
    tree.element_set_style(
        back,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::ZIndex(0),
        ],
    );
    tree.element_set_style(
        front,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::MarginTop(Dimension::px(-50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            StyleProp::ZIndex(1),
        ],
    );
    tree
}

fn check_z_index(data: &[u8]) {
    let px = pixel(data, CANVAS_W, 25, 25);
    assert_channel_min(px, 2, 150, "z-index top blue over red");
}

fn build_flex_grow() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = flex_row_root(&mut tree, 80);
    let a = child_view(&mut tree, 81);
    let b = child_view(&mut tree, 82);
    for child in [a, b] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(20.0)),
                StyleProp::Height(Dimension::px(20.0)),
                StyleProp::FlexGrow(1.0),
                StyleProp::BackgroundColor(Color::new(1.0, 0.5, 0.0, 1.0)),
            ],
        );
    }
    tree
}

fn check_flex_grow(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 60, 5), "flex-grow expanded second child");
}

fn build_flex_shrink() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = flex_row_root(&mut tree, 90);
    let a = child_view(&mut tree, 91);
    let b = child_view(&mut tree, 92);
    for (child, shrink) in [(a, 0.0), (b, 1.0)] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(80.0)),
                StyleProp::Height(Dimension::px(20.0)),
                StyleProp::FlexShrink(shrink),
                StyleProp::BackgroundColor(Color::new(1.0, 0.5, 0.0, 1.0)),
            ],
        );
    }
    tree
}

fn check_flex_shrink(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 50, 5), "flex-shrink child visible");
    assert_not_clear(pixel(data, CANVAS_W, 90, 5), "flex-shrink sibling visible");
}

fn build_flex_basis() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = flex_row_root(&mut tree, 93);
    let child = child_view(&mut tree, 94);
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::FlexBasis(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(20.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.5, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_flex_basis(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 30, 5), "flex-basis child visible");
    assert_clear(pixel(data, CANVAS_W, 70, 5), "flex-basis width respected");
}

fn build_align_self() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 95);
    let child = child_view(&mut tree, 96);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(20.0)),
            StyleProp::AlignSelf(AlignSelfValue::FlexEnd),
            StyleProp::BackgroundColor(Color::new(1.0, 0.5, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_align_self(data: &[u8]) {
    assert_clear(pixel(data, CANVAS_W, 10, 10), "align-self left margin clear");
    assert_not_clear(pixel(data, CANVAS_W, 70, 10), "align-self child at cross-axis flex-end");
}

fn build_align_content() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 97);
    let child = child_view(&mut tree, 98);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::AlignContent(AlignContentValue::Center),
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(20.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.5, 0.0, 1.0)),
        ],
    );
    tree
}

fn check_align_content(data: &[u8]) {
    assert_not_clear(pixel(data, CANVAS_W, 10, 10), "align-content child visible");
}

fn build_flex_wrap() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 99);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::FlexWrap(FlexWrapValue::Wrap),
            StyleProp::Width(Dimension::px(70.0)),
            StyleProp::Height(Dimension::px(VH)),
        ],
    );
    let colors = [
        Color::new(1.0, 0.0, 0.0, 1.0),
        Color::new(0.0, 1.0, 0.0, 1.0),
        Color::new(0.0, 0.0, 1.0, 1.0),
    ];
    for (i, color) in colors.into_iter().enumerate() {
        let child = child_view(&mut tree, 100 + i as u64);
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(40.0)),
                StyleProp::Height(Dimension::px(15.0)),
                StyleProp::BackgroundColor(color),
            ],
        );
    }
    tree
}

fn check_flex_wrap(data: &[u8]) {
    let first = pixel(data, CANVAS_W, 10, 5);
    assert_channel_min(first, 0, 150, "flex-wrap first row red");
    let wrapped = pixel(data, CANVAS_W, 10, 20);
    assert_channel_min(wrapped, 1, 150, "flex-wrap second row green");
}

/// Every entry in `style_tags.json` / `HAYATE_CSS_CATALOG`.
/// Border / focus-ring rasterisation regressions (issue #337). Run on both
/// backends so the contract — a 1px border draws as an opaque column, and a
/// focus ring never erases the content under it — holds for tiny-skia and vello.
pub static BORDER_RASTER_CASES: &[CssPixelCase] = &[
    CssPixelCase {
        css_property: "focus-ring-over-fill",
        build: build_focus_ring_over_fill,
        check: check_focus_ring_over_fill,
    },
    CssPixelCase {
        css_property: "border-hairline-1px",
        build: build_border_hairline_1px,
        check: check_border_hairline_1px,
    },
];

pub static CSS_PIXEL_CASES: &[CssPixelCase] = &[
    CssPixelCase {
        css_property: "background-color",
        build: build_background_color,
        check: check_background_color,
    },
    CssPixelCase {
        css_property: "opacity",
        build: build_opacity,
        check: check_opacity,
    },
    CssPixelCase {
        css_property: "border-radius",
        build: build_border_radius,
        check: check_border_radius,
    },
    CssPixelCase {
        css_property: "border-width",
        build: build_border_width,
        check: check_border_width,
    },
    CssPixelCase {
        css_property: "border-color",
        build: build_border_color,
        check: check_border_color,
    },
    CssPixelCase {
        css_property: "width",
        build: build_width,
        check: check_width,
    },
    CssPixelCase {
        css_property: "height",
        build: build_height,
        check: check_height,
    },
    CssPixelCase {
        css_property: "min-width",
        build: build_min_width,
        check: check_min_width,
    },
    CssPixelCase {
        css_property: "min-height",
        build: build_min_height,
        check: check_min_height,
    },
    CssPixelCase {
        css_property: "max-width",
        build: build_max_width,
        check: check_max_width,
    },
    CssPixelCase {
        css_property: "max-height",
        build: build_max_height,
        check: check_max_height,
    },
    CssPixelCase {
        css_property: "display",
        build: build_display_flex,
        check: check_display_flex,
    },
    CssPixelCase {
        css_property: "display-none",
        build: build_display_none,
        check: check_display_none,
    },
    CssPixelCase {
        css_property: "display-grid",
        build: build_display_grid,
        check: check_display_grid,
    },
    CssPixelCase {
        css_property: "grid-template-columns",
        build: build_grid_template_columns_fr,
        check: check_grid_template_columns_fr,
    },
    CssPixelCase {
        css_property: "grid-template-columns-px",
        build: build_grid_template_columns_px,
        check: check_grid_template_columns_px,
    },
    CssPixelCase {
        css_property: "flex-direction",
        build: build_flex_direction,
        check: check_flex_direction,
    },
    CssPixelCase {
        css_property: "align-items",
        build: build_align_items,
        check: check_align_items,
    },
    CssPixelCase {
        css_property: "justify-content",
        build: build_justify_content,
        check: check_justify_content,
    },
    CssPixelCase {
        css_property: "gap",
        build: build_gap,
        check: check_gap,
    },
    CssPixelCase {
        css_property: "padding",
        build: build_padding,
        check: check_padding,
    },
    CssPixelCase {
        css_property: "padding-top",
        build: build_padding_top,
        check: check_padding_top,
    },
    CssPixelCase {
        css_property: "padding-right",
        build: build_padding_right,
        check: check_padding_right,
    },
    CssPixelCase {
        css_property: "padding-bottom",
        build: build_padding_bottom,
        check: check_padding_bottom,
    },
    CssPixelCase {
        css_property: "padding-left",
        build: build_padding_left,
        check: check_padding_left,
    },
    CssPixelCase {
        css_property: "margin",
        build: build_margin,
        check: check_margin,
    },
    CssPixelCase {
        css_property: "margin-top",
        build: build_margin_top,
        check: check_margin_top,
    },
    CssPixelCase {
        css_property: "margin-right",
        build: build_margin_right,
        check: check_margin_right,
    },
    CssPixelCase {
        css_property: "margin-bottom",
        build: build_margin_bottom,
        check: check_margin_bottom,
    },
    CssPixelCase {
        css_property: "margin-left",
        build: build_margin_left,
        check: check_margin_left,
    },
    CssPixelCase {
        css_property: "font-size",
        build: build_font_size,
        check: check_font_size,
    },
    CssPixelCase {
        css_property: "color",
        build: build_color,
        check: check_color,
    },
    CssPixelCase {
        css_property: "font-family",
        build: build_font_family,
        check: check_font_family,
    },
    CssPixelCase {
        css_property: "font-weight",
        build: build_font_weight,
        check: check_font_weight,
    },
    CssPixelCase {
        css_property: "text-decoration-underline",
        build: build_text_decoration_underline,
        check: check_text_decoration_underline,
    },
    CssPixelCase {
        css_property: "text-decoration-line-through",
        build: build_text_decoration_line_through,
        check: check_text_decoration_line_through,
    },
    CssPixelCase {
        css_property: "z-index",
        build: build_z_index,
        check: check_z_index,
    },
    CssPixelCase {
        css_property: "flex-grow",
        build: build_flex_grow,
        check: check_flex_grow,
    },
    CssPixelCase {
        css_property: "flex-shrink",
        build: build_flex_shrink,
        check: check_flex_shrink,
    },
    CssPixelCase {
        css_property: "flex-basis",
        build: build_flex_basis,
        check: check_flex_basis,
    },
    CssPixelCase {
        css_property: "align-self",
        build: build_align_self,
        check: check_align_self,
    },
    CssPixelCase {
        css_property: "align-content",
        build: build_align_content,
        check: check_align_content,
    },
    CssPixelCase {
        css_property: "flex-wrap",
        build: build_flex_wrap,
        check: check_flex_wrap,
    },
    // Appended at the end so existing `css_pixels.rs` index-based tests keep their offsets.
    CssPixelCase {
        css_property: "border-style",
        build: build_border_style,
        check: check_border_style,
    },
    CssPixelCase {
        css_property: "overflow",
        build: build_overflow_hidden,
        check: check_overflow_hidden,
    },
    CssPixelCase {
        css_property: "box-shadow",
        build: build_box_shadow,
        check: check_box_shadow,
    },
    CssPixelCase {
        css_property: "box-shadow-inset",
        build: build_box_shadow_inset,
        check: check_box_shadow_inset,
    },
];

pub fn render_tree_to_scene(mut tree: ElementTree) -> hayate_core::SceneGraph {
    tree.render(0.0).clone()
}

#[cfg(test)]
mod catalog_coverage {
    use super::CSS_PIXEL_CASES;

    /// `display` has flex/none/grid variants — 34 cases for 32 catalog props.
    const CATALOG_PROPERTIES: &[&str] = &[
        "background-color",
        "opacity",
        "border-radius",
        "border-width",
        "border-color",
        "border-style",
        "box-shadow",
        "overflow",
        "width",
        "height",
        "min-width",
        "min-height",
        "max-width",
        "max-height",
        "display",
        "flex-direction",
        "align-items",
        "justify-content",
        "gap",
        "padding",
        "padding-top",
        "padding-right",
        "padding-bottom",
        "padding-left",
        "margin",
        "margin-top",
        "margin-right",
        "margin-bottom",
        "margin-left",
        "font-size",
        "color",
        "font-family",
        "font-weight",
        "text-decoration",
        "z-index",
        "flex-grow",
        "flex-shrink",
        "flex-basis",
        "align-self",
        "align-content",
        "flex-wrap",
    ];

    #[test]
    fn every_catalog_property_has_pixel_case() {
        for prop in CATALOG_PROPERTIES {
            assert!(
                CSS_PIXEL_CASES.iter().any(|c| {
                    c.css_property == *prop
                        || (c.css_property == "display-none" && *prop == "display")
                        || (c.css_property == "display-grid" && *prop == "display")
                        || (c.css_property.starts_with("text-decoration-") && *prop == "text-decoration")
                }),
                "missing pixel case for {prop}"
            );
        }
        assert!(
            CSS_PIXEL_CASES.len() >= CATALOG_PROPERTIES.len(),
            "expected at least {} cases, got {}",
            CATALOG_PROPERTIES.len(),
            CSS_PIXEL_CASES.len()
        );
    }
}
