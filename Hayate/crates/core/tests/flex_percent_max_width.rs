//! 回帰: パーセント `max-width` を持つ flex 列コンテナの**高さ**（本軸）固有サイズ。
//!
//! `width:620 + max-width:100%`（= min(親, 620)）のカードは、狭い親では幅 360 にクランプ
//! される。以前は taffy の flex base size 計算が、子の既知クロスサイズに**クランプ前の
//! style 幅 620** を使っていたため、折り返しテキストを広い幅(行数少→低い)で測り、列コンテナ
//! の高さを過小評価していた。結果、末尾の子（区切り線・フッター）がカード下端からはみ出した
//! （「白いコンテナが下まで伸びてこない」不具合）。DOM はこの CSS を正しく描くので、Canvas も
//! 一致しなければならない（Hayate CSS の役割）。
//!
//! 修正: `determine_flex_base_size` の `child_known_dimensions` のクロスサイズを item の
//! min/max クロスサイズでクランプする（`crates/vendor/taffy/src/compute/flexbox.rs`）。

use hayate_core::{
    Color, Dimension, DisplayValue, ElementId, ElementKind, ElementTree, FlexDirectionValue,
    StyleProp,
};

/// 折り返す段落 + 固定高フッターを持つ列パネルを、指定の width/max-width で組み、
/// (panel_bottom, footer_bottom, panel_width) を返す。
fn layout_panel(width: StyleProp, max_width: Option<StyleProp>) -> (f32, f32, f32) {
    let mut tree = ElementTree::new();
    tree.register_font("Inter", include_bytes!("../assets/fonts/NotoSansJP.ttf").to_vec());
    let mut next = 1u64;
    let mut mk = |tree: &mut ElementTree, kind, styles: &[StyleProp]| {
        let id = tree.element_create(next, kind);
        next += 1;
        tree.element_set_style(id, styles);
        id
    };

    let root = mk(&mut tree, ElementKind::View, &[
        StyleProp::Width(Dimension::px(360.0)),
        StyleProp::Height(Dimension::px(1000.0)),
        StyleProp::Display(DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::DefaultFontFamily("Inter".to_string()),
        StyleProp::DefaultColor(Color::BLACK),
        StyleProp::DefaultFontSize(13.0),
    ]);
    tree.set_root(root);
    tree.set_viewport(360.0, 1000.0);

    let mut panel_styles = vec![
        width,
        StyleProp::Display(DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::Gap(Dimension::px(8.0)),
        StyleProp::Padding(Dimension::px(14.0)),
        StyleProp::BackgroundColor(Color::new(0.99, 0.99, 0.98, 1.0)),
    ];
    if let Some(mw) = max_width {
        panel_styles.insert(1, mw);
    }
    let panel = mk(&mut tree, ElementKind::View, &panel_styles);
    tree.element_append_child(root, panel);

    let para = mk(&mut tree, ElementKind::Text, &[StyleProp::FontSize(13.0)]);
    tree.element_set_text(
        para,
        "この段落は選択できます。ダブルクリックで単語、トリプルクリックで段落を選び、Shift+クリックや Shift+矢印で範囲を伸縮、Cmd/Ctrl+A で全選択できます。とても長い文章。",
    );
    tree.element_append_child(panel, para);
    let footer = mk(&mut tree, ElementKind::View, &[StyleProp::Height(Dimension::px(30.0))]);
    tree.element_append_child(panel, footer);

    tree.render(0.0);
    let r = |id: ElementId| tree.element_layout_rect(id).unwrap();
    let pr = r(panel);
    let fr = r(footer);
    (pr.1 + pr.3, fr.1 + fr.3, pr.2)
}

/// 不具合の正本パターン: `width:620 + max-width:100%`。親 360 にクランプされ、かつ
/// 折り返し段落 + フッターを正しく内包する（フッター下端 + 下パディングがカード内）。
#[test]
fn width_px_maxwidth_percent_contains_all_children() {
    let (panel_bottom, footer_bottom, panel_w) = layout_panel(
        StyleProp::Width(Dimension::px(620.0)),
        Some(StyleProp::MaxWidth(Dimension::percent(100.0))),
    );
    assert!((panel_w - 360.0).abs() < 0.5, "panel clamps to parent width, got {panel_w}");
    assert!(
        footer_bottom + 14.0 <= panel_bottom + 0.5,
        "footer (+bottom padding) must sit inside the panel: footer_bottom={footer_bottom}, panel_bottom={panel_bottom}",
    );
}

/// 同値の `width:100% + max-width:620px` も当然内包する（順序非依存）。
#[test]
fn width100_maxwidth_px_contains_all_children() {
    let (panel_bottom, footer_bottom, panel_w) = layout_panel(
        StyleProp::Width(Dimension::percent(100.0)),
        Some(StyleProp::MaxWidth(Dimension::px(620.0))),
    );
    assert!((panel_w - 360.0).abs() < 0.5, "panel clamps to parent width, got {panel_w}");
    assert!(footer_bottom + 14.0 <= panel_bottom + 0.5);
}

/// 対照: 固定幅でも当然内包する。
#[test]
fn fixed_width_contains_all_children() {
    let (panel_bottom, footer_bottom, _) =
        layout_panel(StyleProp::Width(Dimension::px(360.0)), None);
    assert!(footer_bottom + 14.0 <= panel_bottom + 0.5);
}
