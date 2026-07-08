//! 回帰: `min-width`/`max-width` でクランプされる column カードを wrap 行に置くと、
//! 行の cross size（高さ）がクランプ**前**の幅で計測した 1 行分に固定され、
//! カードが行からはみ出して後続の兄弟（次セクションの見出し等）と重なる
//! （todo CSS Gallery の「Motion」見出しが直前カードに重なるバグ、canvas モードのみ）。
//!
//! 原因は vendored taffy 0.7.7（upstream 0.9.2 でも未修正）の flexbox `ComputeSize`:
//! 単一行コンテナの cross size が min/max クランプで縮むとき、main size はクランプ前の
//! cross 空間で計測した値のまま返るため、(幅, 高さ) が自己矛盾した組になる。この組が
//! Taffy のノードキャッシュ（`known.width == cached_size.width` を一致とみなす）経由で
//! 「その幅での高さ」として再利用され、wrap 行の hypothetical cross size を 1 行分に
//! 誤らせる。修正は `crates/vendor/taffy/src/compute/flexbox.rs`（クランプが縮めた場合は
//! クランプ後 cross を既知次元にしてサイズ計算をやり直す）。
//!
//! このテストは Hayate を介さない素の taffy 再現（パッチの最小シーム）。ElementTree
//! 経由のギャラリー同型再現は `gallery_wrap_section_overlap.rs`。

use taffy::prelude::*;
use taffy::{AvailableSpace, Size as TaffySize};

/// テキスト風 measure: Hayate の実測応答（NotoSansJP 13px・ギャラリーのカードタイトル）を
/// 単純化したルックアップ。幅 w に対して折り返し行数が増える、典型的な wrapping text。
fn title_measure(
    known: TaffySize<Option<f32>>,
    available: TaffySize<AvailableSpace>,
) -> TaffySize<f32> {
    let max_advance = match known.width {
        Some(w) => Some(w),
        None => match available.width {
            AvailableSpace::Definite(w) => Some(w),
            AvailableSpace::MaxContent => None,
            AvailableSpace::MinContent => Some(0.0),
        },
    };
    let (w, h) = match max_advance {
        None => (426.9849, 18.824),
        Some(w) if w == 0.0 => (112.866005, 131.76799),
        Some(w) if w >= 426.0 => (426.9849, 18.824),
        Some(w) => {
            let lines = (426.9849f32 / w).ceil().max(1.0);
            (w.min(426.9849), lines * 18.824)
        }
    };
    TaffySize { width: w, height: h }
}

#[test]
fn wrap_row_line_cross_size_accounts_for_minmax_clamped_card() {
    let mut taffy: TaffyTree<()> = TaffyTree::new();

    let text = taffy.new_leaf_with_context(Style::default(), ()).unwrap();

    // PopCard 同型: column, gap 12, min-width 200 / max-width 268, padding 16。
    let card = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                gap: Size {
                    width: LengthPercentage::Length(12.0),
                    height: LengthPercentage::Length(12.0),
                },
                min_size: Size { width: Dimension::Length(200.0), height: Dimension::Auto },
                max_size: Size { width: Dimension::Length(268.0), height: Dimension::Auto },
                padding: Rect::length(16.0),
                ..Default::default()
            },
            &[text],
        )
        .unwrap();

    // SectionView の cards 行同型: flex wrap, gap 14。
    let row = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_wrap: FlexWrap::Wrap,
                gap: Size {
                    width: LengthPercentage::Length(14.0),
                    height: LengthPercentage::Length(14.0),
                },
                ..Default::default()
            },
            &[card],
        )
        .unwrap();

    // モバイル幅の App shell 同型: column 412x892。
    let shell = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size { width: Dimension::Length(412.0), height: Dimension::Length(892.0) },
                ..Default::default()
            },
            &[row],
        )
        .unwrap();

    taffy
        .compute_layout_with_measure(
            shell,
            TaffySize {
                width: AvailableSpace::Definite(412.0),
                height: AvailableSpace::Definite(892.0),
            },
            |known, available, node, _ctx, _style| {
                if node == text {
                    title_measure(known, available)
                } else {
                    TaffySize::ZERO
                }
            },
        )
        .unwrap();

    let row_l = taffy.layout(row).unwrap();
    let card_l = taffy.layout(card).unwrap();

    // カードは max-width 268 にクランプされ、タイトルは 2 行に折り返す。
    assert!((card_l.size.width - 268.0).abs() < 0.5, "card width was {}", card_l.size.width);
    assert!(
        card_l.size.height <= row_l.size.height + 0.5,
        "card (h {}) must fit in its wrap row (h {}) — row line cross size must be measured at the clamped width",
        card_l.size.height,
        row_l.size.height
    );
}
