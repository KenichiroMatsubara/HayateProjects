//! スクロールバー chrome のポインタモダリティ分岐（ADR-0110）。選択 chrome を
//! ゲートするのと同じモダリティ軸（ADR-0104, `selection_chrome_modality.rs`）が
//! スクロールバーオーバーレイを二分する。Mouse/Pen は操作可能なつまみを得て、
//! Touch はスクロール中に現れ停止後にフェードする操作不能な一時インジケータを得る。
//! インジケータはつまみ/トラックのヒット領域を持たない — コンテンツのフリックで
//! スクロールするのであり、ドラッグではない。
//!
//! 公開 `ElementTree` インターフェース（ポインタ配線 `on_pointer_*_with_kind`、
//! Scroll Offset シーム、描画済み SceneGraph）経由で駆動し、lowering 内部には
//! 触れない。

use hayate_core::element::pointer::PointerKind;
use hayate_core::element::scene_build::{
    SCROLLBAR_INDICATOR_COLOR, SCROLLBAR_INDICATOR_FADE_MS, SCROLLBAR_INDICATOR_HOLD_MS,
    SCROLLBAR_INDICATOR_OPACITY, SCROLLBAR_INDICATOR_THICKNESS, SCROLLBAR_THICKNESS,
    SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_OPACITY,
};
use hayate_core::{Color, Dimension, ElementId, ElementKind, ElementTree, NodeKind, StyleProp};

/// 合成後の操作可能つまみの塗り色（オーバーレイ不透明度での RGB）。
fn thumb_rgba() -> [f32; 4] {
    SCROLLBAR_THUMB_COLOR
        .with_opacity(SCROLLBAR_THUMB_OPACITY)
        .to_array_f32()
}

/// 操作可能な Mouse/Pen つまみ矩形 `(x, y, w, h)`。インタラクティブバーが描く、
/// 交差軸 THICKNESS のつまみ塗り色。
fn operable_thumbs(tree: &ElementTree) -> Vec<(f32, f32, f32, f32)> {
    let rgba = thumb_rgba();
    tree.scene_graph()
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                ..
            } if *color == rgba
                && ((*width - SCROLLBAR_THICKNESS).abs() < 0.01
                    || (*height - SCROLLBAR_THICKNESS).abs() < 0.01) =>
            {
                Some((*x, *y, *width, *height))
            }
            _ => None,
        })
        .collect()
}

/// Touch 一時インジケータの矩形 `(x, y, w, h)`。（フェード中の）不透明度に関わらず、
/// 交差軸 INDICATOR_THICKNESS のインジケータ色。
fn vertical_indicators(tree: &ElementTree) -> Vec<(f32, f32, f32, f32)> {
    let rgb = SCROLLBAR_INDICATOR_COLOR.to_array_f32();
    tree.scene_graph()
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                ..
            } if (color[0], color[1], color[2]) == (rgb[0], rgb[1], rgb[2])
                && color[3] > 0.0
                && (*width - SCROLLBAR_INDICATOR_THICKNESS).abs() < 0.01 =>
            {
                Some((*x, *y, *width, *height))
            }
            _ => None,
        })
        .collect()
}

/// （単一の）垂直インジケータ矩形のアルファ。描画がなければ `None`。
fn indicator_alpha(tree: &ElementTree) -> Option<f32> {
    let rgb = SCROLLBAR_INDICATOR_COLOR.to_array_f32();
    tree.scene_graph().iter().find_map(|(_, n)| match &n.kind {
        NodeKind::Rect { width, color, .. }
            if (color[0], color[1], color[2]) == (rgb[0], rgb[1], rgb[2])
                && color[3] > 0.0
                && (*width - SCROLLBAR_INDICATOR_THICKNESS).abs() < 0.01 =>
        {
            Some(color[3])
        }
        _ => None,
    })
}

/// 垂直軸のみコンテンツがあふれる `scroll-view`。100×100 のボックスに 100×300 の
/// コンテンツを入れる。`(tree, scroll_id)` を返す。
fn vertical_overflow_scroll_view() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

/// アクティブなポインタモダリティを Touch にし（コンテンツのフリックは Touch
/// ジェスチャ）、Scroll Offset シーム経由でコンテンツをスクロールし、`now_ms` で描画。
fn touch_scroll_at(tree: &mut ElementTree, scroll: ElementId, now_ms: f64) {
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    tree.on_pointer_up_with_kind(10.0, 10.0, PointerKind::Touch);
    tree.element_set_scroll_offset(scroll, 0.0, 40.0);
    tree.render(now_ms);
}

#[test]
fn touch_scroll_draws_no_operable_thumb() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    // デフォルトモダリティは Mouse: 操作可能なつまみが描かれる。
    assert_eq!(
        operable_thumbs(&tree).len(),
        1,
        "Mouse modality paints the operable thumb",
    );

    // Touch では操作可能な Mouse/Pen バーを描いてはならない — 代わりに Touch は
    // つかめるつまみのない一時インジケータを得る（ADR-0110）。
    touch_scroll_at(&mut tree, scroll, 0.0);
    assert!(
        operable_thumbs(&tree).is_empty(),
        "Touch modality draws no operable Mouse/Pen thumb",
    );
}

#[test]
fn touch_scroll_shows_transient_indicator() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    // 静止した Touch サーフェスはスクロールバーを描かない — モバイルに常時表示の
    // バーはない。（デフォルトは Mouse なので、まず押下で Touch に切り替える。）
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    tree.on_pointer_up_with_kind(10.0, 10.0, PointerKind::Touch);
    tree.render(0.0);
    assert!(
        vertical_indicators(&tree).is_empty(),
        "a Touch surface that is not scrolling shows no indicator",
    );

    // コンテンツのスクロールで一時インジケータが立つ: あふれる軸に縦バー1本、
    // 操作可能なつまみより細い。
    touch_scroll_at(&mut tree, scroll, 0.0);
    let indicators = vertical_indicators(&tree);
    assert_eq!(
        indicators.len(),
        1,
        "a Touch scroll raises exactly one (vertical) transient indicator",
    );
    assert_eq!(
        indicators[0].2, SCROLLBAR_INDICATOR_THICKNESS,
        "the indicator is INDICATOR_THICKNESS wide",
    );
}

#[test]
fn touch_indicator_fades_out_after_scrolling_stops() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    touch_scroll_at(&mut tree, scroll, 0.0);
    assert_eq!(
        indicator_alpha(&tree),
        Some(SCROLLBAR_INDICATOR_OPACITY),
        "the indicator is fully visible while scrolling",
    );

    // ホールド窓の間、インジケータは完全に見えたまま。
    tree.render(SCROLLBAR_INDICATOR_HOLD_MS / 2.0);
    assert_eq!(
        indicator_alpha(&tree),
        Some(SCROLLBAR_INDICATOR_OPACITY),
        "the indicator holds at full visibility before the fade begins",
    );

    // フェード窓の途中では暗くなるが、まだ描かれている。
    tree.render(SCROLLBAR_INDICATOR_HOLD_MS + SCROLLBAR_INDICATOR_FADE_MS / 2.0);
    let mid = indicator_alpha(&tree).expect("the indicator is still drawn mid-fade");
    assert!(
        mid > 0.0 && mid < SCROLLBAR_INDICATOR_OPACITY,
        "the indicator is fading (0 < {mid} < {SCROLLBAR_INDICATOR_OPACITY})",
    );

    // ホールド + フェードを過ぎると完全に消えてなくなる。
    tree.render(SCROLLBAR_INDICATOR_HOLD_MS + SCROLLBAR_INDICATOR_FADE_MS + 100.0);
    assert!(
        vertical_indicators(&tree).is_empty(),
        "the indicator fades out and disappears once scrolling stops",
    );
}

/// 精密ポインタ（Mouse/Pen）ではオーバーレイは操作可能なつまみになる。描画され、
/// 一時インジケータを持たず、押下 + ドラッグで Scroll Offset を動かす。`PointerKind`
/// でパラメタライズした精密ポインタ側の検証。
fn assert_precise_pointer_is_operable(kind: PointerKind) {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, kind);
    tree.on_pointer_up_with_kind(10.0, 10.0, kind);
    tree.render(0.0);

    let thumbs = operable_thumbs(&tree);
    assert_eq!(thumbs.len(), 1, "{kind:?} paints the operable thumb");
    assert!(
        vertical_indicators(&tree).is_empty(),
        "{kind:?} paints no transient indicator",
    );

    let (tx, ty, tw, th) = thumbs[0];
    let (cx, cy) = (tx + tw / 2.0, ty + th / 2.0);
    tree.on_pointer_down_with_kind(cx, cy, 0, kind);
    tree.on_pointer_move(cx, cy + 20.0);
    assert!(
        tree.element_get_scroll_offset(scroll).1 > 0.0,
        "a {kind:?} drag on the thumb operates the scrollbar",
    );
}

#[test]
fn mouse_and_pen_get_an_operable_thumb() {
    assert_precise_pointer_is_operable(PointerKind::Mouse);
    assert_precise_pointer_is_operable(PointerKind::Pen);
}

#[test]
fn touch_gets_a_non_operable_indicator() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    touch_scroll_at(&mut tree, scroll, 0.0);

    // Touch は一時インジケータのみ描く — 操作可能な Mouse/Pen つまみは描かない。
    let indicators = vertical_indicators(&tree);
    assert_eq!(indicators.len(), 1, "Touch draws the transient indicator");
    assert!(
        operable_thumbs(&tree).is_empty(),
        "Touch draws no operable thumb",
    );

    // インジケータはつまみ/トラックのヒット領域を持たない: そのピクセル上での
    // 押下 + ドラッグはスクロールバーを操作しない（実フリックはバーでなくコンテンツを
    // スクロールする）。
    let (ix, iy, iw, ih) = indicators[0];
    let (cx, cy) = (ix + iw / 2.0, iy + ih / 2.0);
    let before = tree.element_get_scroll_offset(scroll).1;
    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Touch);
    tree.on_pointer_move(cx, cy + 30.0);
    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        before,
        "the Touch indicator has no hit region — the press operates no scrollbar",
    );
}
