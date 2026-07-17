//! `text-input` の UA デフォルト幅(ADR-0109)。
//!
//! 明示的な `width` を持たない `text-input` は、Canvas パスでパディングだけの幅に
//! 潰れないよう、フォント相対の固有コンテンツ幅(ブラウザ `<input size=20>` デフォルト)を
//! 持たねばならない。これらのテストは両 Scene Renderer が観測するのと同じ公開ドキュメント
//! API(`render` + `element_layout_rect`)経由で挙動を駆動する。

use hayate_core::{
    AlignValue, BorderStyleValue, Color, Dimension, DisplayValue, ElementId, ElementKind,
    ElementTree, FlexDirectionValue, StyleProp,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// `input_style()` のボーダー + 水平パディング: パディング 12+12、ボーダー 1+1。
const INPUT_CHROME_PX: f32 = 26.0;

fn input_style() -> Vec<StyleProp> {
    // theme.ts の `inputStyle()` を反映 — width なし、flex-grow なし。
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

/// 単一の `text-input` を持つギャラリーの `PopCard` コンテナ(列 flex、
/// `align-items: flex-start`)を構築・描画し、入力のボーダーボックス幅を返す。
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

/// `input_style()` フィールドのコンテンツ幅(ボーダーボックスからクロームを除いた値)。
fn input_content_width(placeholder: &str) -> f32 {
    input_border_box_width(&input_style(), placeholder) - INPUT_CHROME_PX
}

#[test]
fn width_unspecified_text_input_gets_font_relative_default_width() {
    // UA デフォルトが無いと入力はコンテンツ幅 ~0 に潰れる(プレースホルダが1文字/行で折り返す)。
    // あれば 13px の ~20 文字が1行に収まり、50px を十分に超える。
    let content_width = input_content_width("Type here");
    assert!(
        content_width > 50.0,
        "width-unspecified text-input must carry a non-trivial default width, got {content_width}"
    );
}

#[test]
fn default_width_scales_with_font_size() {
    // UA デフォルトは現在のフォントでの N 文字分なので、font-size が大きいほど
    // フィールドは比例して広がる(固定 px ではなくブラウザ `<input>` の挙動)。
    let small = input_border_box_width(
        &[
            StyleProp::Height(Dimension::px(38.0)),
            StyleProp::FontSize(13.0),
        ],
        "x",
    );
    let large = input_border_box_width(
        &[
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::FontSize(26.0),
        ],
        "x",
    );
    // font-size を倍にすればデフォルト幅もおおむね倍になるはず。
    assert!(
        large > small * 1.7,
        "default width must follow font-size: 13px gave {small}, 26px gave {large}"
    );
}

#[test]
fn explicit_width_overrides_default() {
    // 明示的な `width` は UA デフォルトより優先される(Taffy の固有サイズ順:
    // 明示 > 要素種別デフォルト)。フィールドは要求どおりの幅になる。
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
    // addform のケース: 行内で `flex-grow:1` を持つ入力は ~20 文字デフォルトで止まらず
    // 行を埋めるよう伸びる(Taffy の固有サイズ順: grow が勝つ)。
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
    // 300px の行で唯一の flex-grow 子は行を埋める: ~300px ≫ 20ch。
    assert!(
        iw > 280.0,
        "flex-grow:1 input must fill the row past the default width, got {iw}"
    );
}

#[test]
fn default_width_is_independent_of_text_content() {
    // ブラウザ `<input size>` デフォルトはフィールド幅を固定して値をスクロールし、
    // 内容に合わせて伸びない。短いプレースホルダと長いプレースホルダは同じデフォルト幅になる。
    let short = input_content_width("a");
    let long = input_content_width("a very long placeholder that far exceeds twenty characters");
    assert!(
        (short - long).abs() < 0.5,
        "default width must not depend on text content: short={short}, long={long}"
    );
}
