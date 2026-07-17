//! 回帰: `align-items: baseline` の flex 行に置いた `text` 子が、ボックス幅は正しいのに
//! グリフだけ min-content（1 文字/単語ごと）で折り返す不具合（react-todo のタイトル折れ）。
//!
//! 原因: measure クロージャは Taffy のサイズ解決中に何度も呼ばれ、retained text_layout は
//! 最後の呼び出しのシェイプを保持する（last-wins）。`align-items: baseline` の行では、確定
//! サイズの後にも intrinsic-size プローブが走り、その最後が MinContent（max_advance=0）になって
//! 折り返したグリフ列が残る。ボックス幾何は確定サイズの測定からキャッシュされ正しいため、
//! 正しい幅のボックスへ min-content 折返しのグリフが描かれ縦にはみ出す。DOM レンダラーは
//! ブラウザ flex で正しく描くので Canvas も一致しなければならない（Hayate CSS の役割）。
//!
//! 修正: compute_layout 後、各テキストを Taffy の確定（unrounded）インナー幅へ揃え直して
//! 再シェイプする（`crates/core/src/element/layout_pass.rs`）。

use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, FlexDirectionValue,
    StyleProp,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// react-todo のカード冒頭（page=scroll-view > card > title-row）を最小再現し、
/// タイトル行の `align-items` を引数にして (title, sub) の行数を返す。
fn title_row_line_counts(align: AlignValue) -> (Option<usize>, Option<usize>) {
    let mut tree = ElementTree::new();
    // WASM バンドル相当のフォント文脈（system_fonts なし、バンドル既定のみ）。
    tree.test_set_wasm_like_fonts(FONT.to_vec());
    let mut next = 1u64;
    let mut mk = |t: &mut ElementTree, k: ElementKind, s: &[StyleProp]| {
        let id = t.element_create(next, k);
        next += 1;
        t.element_set_style(id, s);
        id
    };
    let ink = Color::new(0.9, 0.93, 0.97, 1.0);

    let shell = mk(
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
        ],
    );
    tree.set_root(shell);
    tree.set_viewport(1280.0, 720.0);

    // page: scroll-view, alignItems:center（card は中央寄せ、cross 方向は stretch しない）。
    let page = mk(
        &mut tree,
        ElementKind::ScrollView,
        &[
            StyleProp::FlexGrow(1.0),
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::PaddingTop(Dimension::px(36.0)),
        ],
    );
    tree.element_append_child(shell, page);

    // card: width 520 / maxWidth 100% の column。
    let card = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(520.0)),
            StyleProp::MaxWidth(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(16.0)),
            StyleProp::Padding(Dimension::px(22.0)),
        ],
    );
    tree.element_append_child(page, card);

    // title-row: row, align-items=引数, gap 10。子は幅指定なしの text 2 つ（App.tsx と同型）。
    let title_row = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(align),
            StyleProp::Gap(Dimension::px(10.0)),
        ],
    );
    tree.element_append_child(card, title_row);

    let title = mk(
        &mut tree,
        ElementKind::Text,
        &[
            StyleProp::DefaultColor(ink),
            StyleProp::DefaultFontSize(22.0),
            StyleProp::FontWeight(700.0),
        ],
    );
    tree.element_set_text(title, "React TODO");
    tree.element_append_child(title_row, title);

    let sub = mk(
        &mut tree,
        ElementKind::Text,
        &[StyleProp::DefaultFontSize(13.0)],
    );
    tree.element_set_text(sub, "残り 2 / 3 件");
    tree.element_append_child(title_row, sub);

    tree.render(0.0);

    // ボックス幅は十分（card 内 476px）あることを確認しておく（折り返す物理的理由がない）。
    let row_w = tree
        .element_layout_rect(title_row)
        .map(|r| r.2)
        .unwrap_or(0.0);
    assert!(
        row_w > 400.0,
        "title-row must be wide ({row_w}); 折返しの物理的理由はない"
    );

    (
        tree.test_text_line_count(title),
        tree.test_text_line_count(sub),
    )
}

#[test]
fn baseline_row_text_is_not_wrapped_to_min_content() {
    let (title_lines, sub_lines) = title_row_line_counts(AlignValue::Baseline);
    assert_eq!(
        title_lines,
        Some(1),
        "'React TODO' は 1 行に収まるべき（baseline 行で min-content 折返しのグリフが残っていた）"
    );
    assert_eq!(
        sub_lines,
        Some(1),
        "'残り 2 / 3 件' は 1 行に収まるべき（baseline 行で 1 文字/行に折り返していた）"
    );
}

#[test]
fn baseline_and_center_rows_agree() {
    // baseline は cross 軸（縦）整列の選択にすぎず、テキストの折返し幅に影響してはならない。
    let baseline = title_row_line_counts(AlignValue::Baseline);
    let center = title_row_line_counts(AlignValue::Center);
    assert_eq!(
        baseline, center,
        "baseline と center でテキスト行数が一致すべき（baseline={baseline:?}, center={center:?}）"
    );
}
