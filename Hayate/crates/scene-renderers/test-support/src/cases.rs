use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementId, ElementKind, ElementTree,
    FlexDirectionValue, JustifyValue, StyleProp,
};

use crate::pixel::{assert_channel_min, assert_channel_max, assert_clear, assert_not_clear, pixel};
use crate::pixel::CANVAS_W;

const VW: f32 = 100.0;
const VH: f32 = 100.0;

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

fn text_tree(extra: &[StyleProp]) -> ElementTree {
    let mut tree = ElementTree::new();
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

/// Every entry in `style_tags.json` / `HAYATE_CSS_CATALOG`.
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
        css_property: "z-index",
        build: build_z_index,
        check: check_z_index,
    },
    CssPixelCase {
        css_property: "flex-grow",
        build: build_flex_grow,
        check: check_flex_grow,
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
        "z-index",
        "flex-grow",
    ];

    #[test]
    fn every_catalog_property_has_pixel_case() {
        for prop in CATALOG_PROPERTIES {
            assert!(
                CSS_PIXEL_CASES.iter().any(|c| {
                    c.css_property == *prop
                        || (c.css_property == "display-none" && *prop == "display")
                        || (c.css_property == "display-grid" && *prop == "display")
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
