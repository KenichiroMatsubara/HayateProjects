//! Mouse/Pen のスクロールバー操作（ADR-0110）。サムの pointer-down + ドラッグで
//! Scroll Offset を連続移動、トラック余白のクリックで 1 ステップ分ページ送り、
//! サムドラッグが軸端に達したら残りを `apply_wheel_delta` 経由で祖先 ScrollView へ
//! チェーンする（ホイールとのスクロールチェーン整合、ADR-0084）。操作由来の
//! Offset 変更はホイールと同じ Scroll Offset シーム（`element_set_scroll_offset`,
//! ADR-0046）へ収束する。
//!
//! 公開ポインタ API（`on_pointer_down_with_kind` + `on_pointer_move`）経由で駆動し、
//! サムの描画ジオメトリをシーングラフから読み戻して検証する。

use hayate_core::element::pointer::PointerKind;
use hayate_core::element::scene_build::{
    SCROLLBAR_THICKNESS, SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_OPACITY,
};
use hayate_core::{Color, Dimension, ElementId, ElementKind, ElementTree, NodeKind, StyleProp};

/// 合成後のサム塗り色（オーバーレイ不透明度を掛けた RGB）。
fn thumb_rgba() -> [f32; 4] {
    SCROLLBAR_THUMB_COLOR
        .with_opacity(SCROLLBAR_THUMB_OPACITY)
        .to_array_f32()
}

/// 縦スクロールバーのサム矩形 `(x, y, w, h)`（canvas 座標）を、塗り色と
/// THICKNESS の交差軸幅で同定して全件返す。
fn vertical_thumbs(tree: &ElementTree) -> Vec<(f32, f32, f32, f32)> {
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
            } if *color == rgba && (*width - SCROLLBAR_THICKNESS).abs() < 0.01 => {
                Some((*x, *y, *width, *height))
            }
            _ => None,
        })
        .collect()
}

/// 縦サムが厳密に 1 個であることを保証して返す。
fn vertical_thumb(tree: &ElementTree) -> (f32, f32, f32, f32) {
    let thumbs = vertical_thumbs(tree);
    assert_eq!(thumbs.len(), 1, "expected exactly one vertical thumb");
    thumbs[0]
}

/// 縦軸のみオーバーフローする `scroll-view`（100×100 のボックスに 100×300 の
/// コンテンツ）。`(tree, scroll_id)` を返す。
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

#[test]
fn dragging_the_thumb_scrolls_continuously() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let (tx, ty, tw, th) = vertical_thumb(&tree);
    let (cx, cy) = (tx + tw / 2.0, ty + th / 2.0);

    // Mouse でサムを掴む。押下だけではスクロールしない。
    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Mouse);
    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        0.0,
        "pressing the thumb does not by itself move the offset",
    );

    // サムを下へドラッグすると縦 Scroll Offset が増える。
    tree.on_pointer_move(cx, cy + 10.0);
    let after_first = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_first > 0.0,
        "dragging the thumb down moves the scroll offset (got {after_first})",
    );

    // ドラッグ継続でさらに移動。offset はポインタを連続追従する。
    tree.on_pointer_move(cx, cy + 20.0);
    let after_second = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_second > after_first,
        "a continued drag keeps moving the offset ({after_first} -> {after_second})",
    );

    // リリースでドラッグ終了。以降の move はサムを追従しない。
    tree.on_pointer_up(cx, cy + 20.0);
    tree.on_pointer_move(cx, cy + 40.0);
    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        after_second,
        "after release the thumb no longer follows the pointer",
    );
}

#[test]
fn clicking_the_track_pages_the_offset() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let (tx, ty, tw, th) = vertical_thumb(&tree);

    // サムより*下*のトラック押下は、サムを掴まずに offset を前方（終端側）へ
    // ページ送りする。
    let track_x = tx + tw / 2.0;
    tree.on_pointer_down_with_kind(track_x, ty + th + 20.0, 0, PointerKind::Mouse);
    let after_forward = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_forward > 0.0,
        "a track press below the thumb pages the offset forward (got {after_forward})",
    );
    tree.on_pointer_up(track_x, ty + th + 20.0);

    // 再描画でサムを新位置に置いてから、*上*を押して始端側へページ戻し。
    tree.render(0.0);
    let (_, aty, _, _) = vertical_thumb(&tree);
    tree.on_pointer_down_with_kind(track_x, aty / 2.0, 0, PointerKind::Mouse);
    let after_back = tree.element_get_scroll_offset(scroll).1;
    assert!(
        after_back < after_forward,
        "a track press above the thumb pages the offset back ({after_forward} -> {after_back})",
    );
}

/// ネストした scroll-view。外側 200×200 が、スクロール可能な内側 200×100
/// （200×300 の leaf がオーバーフロー）と 200×250 の tail を持つため、外側も
/// 縦にオーバーフローする。`(tree, outer, inner)` を返す。
fn nested_scroll_tree() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let inner = tree.element_create(2, ElementKind::ScrollView);
    let leaf = tree.element_create(3, ElementKind::View);
    let tail = tree.element_create(4, ElementKind::View);
    tree.set_root(outer);
    tree.set_viewport(400.0, 400.0);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, leaf);
    tree.element_append_child(outer, tail);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_set_style(
        leaf,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_style(
        tail,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(250.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.render(0.0);
    (tree, outer, inner)
}

#[test]
fn thumb_drag_chains_to_the_ancestor_at_the_inner_end() {
    let (mut tree, outer, inner) = nested_scroll_tree();

    // 内側サムは 2 つの縦サムのうち短い方（コンテンツのオーバーフローが大きく
    // サムが小さい）。その中心を掴む。
    let mut thumbs = vertical_thumbs(&tree);
    assert_eq!(
        thumbs.len(),
        2,
        "both inner and outer draw a vertical thumb"
    );
    thumbs.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    let (tx, ty, tw, th) = thumbs[0];
    let inner_max = tree.element_scroll_max_offset(inner).1;
    assert!(inner_max > 0.0 && tree.element_scroll_max_offset(outer).1 > 0.0);

    let cx = tx + tw / 2.0;
    let cy = ty + th / 2.0;
    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Mouse);

    // 内側サムの可動域を超えるまで十分にドラッグし、内側の範囲を使い切る。
    tree.on_pointer_move(cx, cy + th + 200.0);

    let inner_y = tree.element_get_scroll_offset(inner).1;
    let outer_y = tree.element_get_scroll_offset(outer).1;
    assert!(
        (inner_y - inner_max).abs() < 1e-3,
        "the inner offset is pinned at its max ({inner_y} vs {inner_max})",
    );
    assert!(
        outer_y > 0.0,
        "the remaining drag chains to the ancestor ScrollView (outer={outer_y})",
    );
}

#[test]
fn touch_press_does_not_operate_the_scrollbar() {
    // Mouse/Pen のスクロールバーは操作可能だが、Touch は非操作の一時インジケータ
    // になる（ADR-0110）。サムのピクセルへの Touch 押下は掴みもスクロールもせず、
    // コンテンツへ素通りする。
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let (tx, ty, tw, th) = vertical_thumb(&tree);
    let (cx, cy) = (tx + tw / 2.0, ty + th / 2.0);

    tree.on_pointer_down_with_kind(cx, cy, 0, PointerKind::Touch);
    tree.on_pointer_move(cx, cy + 30.0);

    assert_eq!(
        tree.element_get_scroll_offset(scroll).1,
        0.0,
        "a Touch press does not drag the thumb (no interactive Mouse/Pen bar)",
    );
}
