//! 回帰: todo の CSS Gallery（canvas モード）で「Motion」セクション見出しが直前の
//! 「defaultColor / …」カード（caption: inherited text defaults）へ重なるバグの
//! ギャラリー同型再現（ElementTree 経由のエンドツーエンドなシーム）。
//!
//! `min-width`/`max-width` でクランプされるカードが wrap 行に入ると、行の cross size が
//! クランプ前の幅で計測したタイトル 1 行分に固定され、カードがセクションをはみ出して
//! 次セクションの見出しと重なっていた。根本原因と最小シームの再現は
//! `flex_wrap_minmax_clamp_reflow.rs`（vendored taffy の flexbox `ComputeSize` 修正）を参照。
//!
//! ここでは SectionView / PopCard / sections.tsx の該当構造だけを組み、レイアウト直後に
//! 「カードはセクション内に収まる」「Motion 見出し行は前カードの下に来る」を検証する。

use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementId, ElementKind, ElementTree,
    FlexDirectionValue, FlexWrapValue, StyleProp,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

fn rect(tree: &ElementTree, id: ElementId) -> (f32, f32, f32, f32) {
    tree.element_layout_rect(id).expect("layout rect")
}

#[test]
fn gallery_motion_heading_does_not_overlap_previous_card() {
    let mut tree = ElementTree::new();
    tree.test_set_wasm_like_fonts(FONT.to_vec());
    let mut next = 1u64;
    fn mk_el(t: &mut ElementTree, next: &mut u64, k: ElementKind, s: &[StyleProp]) -> ElementId {
        let id = t.element_create(*next, k);
        *next += 1;
        t.element_set_style(id, s);
        id
    }
    macro_rules! mk {
        ($t:expr, $k:expr, $s:expr) => {
            mk_el($t, &mut next, $k, $s)
        };
    }
    let ink = Color::new(0.1, 0.1, 0.15, 1.0);

    // App shell（column, 100%×100%）
    let shell = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::DefaultColor(ink),
            StyleProp::DefaultFontSize(14.0),
            StyleProp::DefaultFontFamily("Inter, Segoe UI, system-ui, sans-serif".to_string()),
        ]
    );
    tree.set_root(shell);
    tree.set_viewport(412.0, 892.0);

    // CssGallery: scroll-view column gap28 padding 18/28/28/28
    let page = mk!(
        &mut tree,
        ElementKind::ScrollView,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(28.0)),
            StyleProp::PaddingTop(Dimension::px(18.0)),
            StyleProp::PaddingLeft(Dimension::px(28.0)),
            StyleProp::PaddingRight(Dimension::px(28.0)),
            StyleProp::PaddingBottom(Dimension::px(28.0)),
        ]
    );
    tree.element_append_child(shell, page);

    // SectionView（Text & Typography 相当）: column gap 14
    let section = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(14.0)),
        ]
    );
    tree.element_append_child(page, section);

    // 見出し行: row center gap 10（bar + text 18/600）
    let make_heading =
        |tree: &mut ElementTree, next: &mut u64, label: &str| -> (ElementId, ElementId) {
            let mut mk2 = |t: &mut ElementTree, k: ElementKind, s: &[StyleProp]| {
                let id = t.element_create(*next, k);
                *next += 1;
                t.element_set_style(id, s);
                id
            };
            let row = mk2(
                tree,
                ElementKind::View,
                &[
                    StyleProp::Display(DisplayValue::Flex),
                    StyleProp::FlexDirection(FlexDirectionValue::Row),
                    StyleProp::AlignItems(AlignValue::Center),
                    StyleProp::Gap(Dimension::px(10.0)),
                ],
            );
            let bar = mk2(
                tree,
                ElementKind::View,
                &[
                    StyleProp::Width(Dimension::px(4.0)),
                    StyleProp::Height(Dimension::px(22.0)),
                    StyleProp::BorderRadius(3.0),
                ],
            );
            tree.element_append_child(row, bar);
            let title = mk2(
                tree,
                ElementKind::Text,
                &[StyleProp::FontSize(18.0), StyleProp::FontWeight(600.0)],
            );
            tree.element_set_text(title, label);
            tree.element_append_child(row, title);
            (row, title)
        };

    let (heading1, _) = make_heading(&mut tree, &mut next, "Text & Typography");
    tree.element_append_child(section, heading1);

    // cards wrap row: flex wrap gap 14
    let cards = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexWrap(FlexWrapValue::Wrap),
            StyleProp::Gap(Dimension::px(14.0)),
        ]
    );
    tree.element_append_child(section, cards);

    // PopCard: column gap12 minW200 maxW268 padding16 border1
    let card = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(12.0)),
            StyleProp::MinWidth(Dimension::px(200.0)),
            StyleProp::MaxWidth(Dimension::px(268.0)),
            StyleProp::Padding(Dimension::px(16.0)),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderRadius(16.0),
        ]
    );
    tree.element_append_child(cards, card);

    // chip 行: row center gap8（dot 10x10 + title 13/600）
    let chip_row = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::Gap(Dimension::px(8.0)),
        ]
    );
    tree.element_append_child(card, chip_row);
    let dot = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(10.0)),
            StyleProp::Height(Dimension::px(10.0)),
            StyleProp::BorderRadius(6.0),
        ]
    );
    tree.element_append_child(chip_row, dot);
    let chip_title = mk!(
        &mut tree,
        ElementKind::Text,
        &[StyleProp::FontSize(13.0), StyleProp::FontWeight(600.0),]
    );
    tree.element_set_text(
        chip_title,
        "defaultColor / defaultFontFamily / defaultFontSize / defaultFontWeight",
    );
    tree.element_append_child(chip_row, chip_title);

    // demo body: column gap8 alignItems flex-start padding14 border1
    let body = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(8.0)),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Padding(Dimension::px(14.0)),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderRadius(12.0),
        ]
    );
    tree.element_append_child(card, body);

    // inner: column gap6 padding10 border1 + ambient defaults（serif 18/700）
    let inner = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(6.0)),
            StyleProp::Padding(Dimension::px(10.0)),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderRadius(8.0),
            StyleProp::DefaultColor(Color::new(0.9, 0.5, 0.2, 1.0)),
            StyleProp::DefaultFontFamily("Georgia, serif".to_string()),
            StyleProp::DefaultFontSize(18.0),
            StyleProp::DefaultFontWeight(700.0),
        ]
    );
    tree.element_append_child(body, inner);
    let t1 = mk!(&mut tree, ElementKind::Text, &[]);
    tree.element_set_text(t1, "Inherited text styles");
    tree.element_append_child(inner, t1);
    let t2 = mk!(&mut tree, ElementKind::Text, &[]);
    tree.element_set_text(t2, "Second line inherits defaults");
    tree.element_append_child(inner, t2);

    // caption
    let caption = mk!(&mut tree, ElementKind::Text, &[StyleProp::FontSize(11.0)]);
    tree.element_set_text(caption, "inherited text defaults");
    tree.element_append_child(card, caption);

    // Motion セクション（見出し行だけで重なりを検出できる）
    let section2 = mk!(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(14.0)),
        ]
    );
    tree.element_append_child(page, section2);
    let (heading2, _) = make_heading(&mut tree, &mut next, "Motion");
    tree.element_append_child(section2, heading2);

    tree.render(0.0);

    let (_, sy, _, sh) = rect(&tree, section);
    let (_, cy, _, ch) = rect(&tree, card);
    let (_, my, _, _mh) = rect(&tree, heading2);
    assert!(
        cy + ch <= sy + sh + 0.5,
        "card (bottom {}) must fit inside its section (bottom {})",
        cy + ch,
        sy + sh
    );
    assert!(
        my + 0.5 >= cy + ch,
        "Motion heading (top {my}) must not overlap the previous card (bottom {})",
        cy + ch
    );
}
