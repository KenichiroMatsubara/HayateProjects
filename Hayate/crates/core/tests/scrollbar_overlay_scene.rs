//! スクロールバーオーバーレイ chrome の lowering（ADR-0110）。スクロール可能な
//! `scroll-view` は、オーバーフローする各軸について ScrollView アンカー下に
//! Mouse/Pen 様のサムオーバーレイを描く。サム形状は Scroll Offset とコンテンツ
//! サイズから導出する。Pointer Modality 分岐やレイアウト領域の予約はまだない。
//!
//! 公開 `ElementTree` インタフェース経由で、`RecordingPainter` の DrawOp 列と
//! SceneGraph の `NodeKind` 走査の両方を通して検証する。

use hayate_core::element::scene_build::{
    SCROLLBAR_THICKNESS, SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_OPACITY,
};
use hayate_core::{
    Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, NodeId, NodeKind,
    RecordingPainter, StyleProp, render_scene_graph,
};

/// 合成後のサム塗り色（オーバーレイ不透明度を乗せた RGB）。
fn thumb_rgba() -> [f32; 4] {
    SCROLLBAR_THUMB_COLOR
        .with_opacity(SCROLLBAR_THUMB_OPACITY)
        .to_array_f32()
}

/// シーングラフ中の全スクロールバーサム `Rect` ノードを `(id, x, y, w, h)` で
/// 返す。サム塗り色で識別する。
fn thumb_nodes(tree: &ElementTree) -> Vec<(NodeId, f32, f32, f32, f32)> {
    let sg = tree.scene_graph();
    let rgba = thumb_rgba();
    sg.iter()
        .filter_map(|(id, n)| match &n.kind {
            NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                ..
            } if *color == rgba => Some((id, *x, *y, *width, *height)),
            _ => None,
        })
        .collect()
}

/// 記録された DrawOp 列中のサム塗り op（公開ペインタ経路）。
fn thumb_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    let rgba = thumb_rgba();
    painter
        .ops()
        .iter()
        .filter(|op| matches!(op, DrawOp::FillRect { color, .. } if *color == rgba))
        .cloned()
        .collect()
}

/// アンカーを上方向に辿る: `node` は `scroll` の `ElementAnchor` の子孫か。
fn is_under_scroll_anchor(tree: &ElementTree, node: NodeId, scroll: ElementId) -> bool {
    let sg = tree.scene_graph();
    let mut current = Some(node);
    while let Some(id) = current {
        if let Some(n) = sg.get(id) {
            if matches!(&n.kind, NodeKind::ElementAnchor { element_id } if *element_id == scroll) {
                return true;
            }
        }
        current = sg.parent_of(id);
    }
    false
}

/// 縦軸のみオーバーフローする `scroll-view`: 100×100 のボックスに 100×300 の
/// コンテンツ。`(tree, scroll_id)` を返す。
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
fn scrollable_view_lowers_a_thumb_under_its_anchor() {
    let (tree, scroll) = vertical_overflow_scroll_view();

    let thumbs = thumb_nodes(&tree);
    assert_eq!(
        thumbs.len(),
        1,
        "a vertically-overflowing scroll-view draws exactly one (vertical) thumb"
    );
    let (thumb_id, _, _, width, _) = thumbs[0];
    assert!(
        is_under_scroll_anchor(&tree, thumb_id, scroll),
        "the thumb is lowered under the ScrollView anchor",
    );
    assert_eq!(
        width, SCROLLBAR_THICKNESS,
        "a vertical thumb is SCROLLBAR_THICKNESS wide",
    );
    assert_eq!(
        thumb_ops(&tree).len(),
        1,
        "the public painter records the thumb as one fill op",
    );
}

/// `bw × bh` のボックス内に `cw × ch` の `content` を持つ `scroll-view`。
fn scroll_view_with_content(bw: f32, bh: f32, cw: f32, ch: f32) -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(bw)),
            StyleProp::Height(Dimension::px(bh)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(cw)),
            StyleProp::Height(Dimension::px(ch)),
            // 明示サイズを保つ: flex アイテムは主軸方向にボックスへ収まるよう縮み、
            // テスト対象の横オーバーフローを隠してしまう。
            StyleProp::FlexShrink(0.0),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

#[test]
fn content_that_fits_draws_no_scrollbar() {
    // 両軸でボックスより小さいコンテンツ: スクロール不要、サムなし。
    let (tree, _scroll) = scroll_view_with_content(200.0, 200.0, 100.0, 100.0);
    assert!(
        thumb_nodes(&tree).is_empty(),
        "a scroll-view whose content fits draws no scrollbar",
    );
    assert!(
        thumb_ops(&tree).is_empty(),
        "and the painter records no thumb fill",
    );
}

#[test]
fn only_the_overflowing_axis_is_drawn() {
    // 幅はオーバーフローし高さは収まる: 横サム 1 本（高さ == thickness、下端）、
    // 縦サムなし。
    let (tree, scroll) = scroll_view_with_content(100.0, 100.0, 300.0, 100.0);
    let thumbs = thumb_nodes(&tree);
    assert_eq!(thumbs.len(), 1, "only the overflowing (horizontal) axis is drawn");

    let (_, _, ty, tw, th) = thumbs[0];
    assert_eq!(th, SCROLLBAR_THICKNESS, "a horizontal thumb is THICKNESS tall");
    assert!(tw > th, "and runs along the horizontal axis");
    let (_, sy, _, sh) = tree.element_layout_rect(scroll).unwrap();
    assert!(
        ty + th <= sy + sh + 0.01,
        "the horizontal thumb sits at the bottom edge of the box",
    );
}

#[test]
fn both_axes_overflow_draws_two_thumbs() {
    // 両軸オーバーフロー: 右に縦サム（幅 THICKNESS）、下に横サム（高さ THICKNESS）。
    let (tree, _scroll) = scroll_view_with_content(100.0, 100.0, 300.0, 300.0);
    let thumbs = thumb_nodes(&tree);
    assert_eq!(thumbs.len(), 2, "both overflowing axes are drawn");

    let vertical = thumbs.iter().filter(|t| t.3 == SCROLLBAR_THICKNESS).count();
    let horizontal = thumbs.iter().filter(|t| t.4 == SCROLLBAR_THICKNESS).count();
    assert_eq!(vertical, 1, "exactly one vertical thumb");
    assert_eq!(horizontal, 1, "exactly one horizontal thumb");
}

fn has_ancestor(tree: &ElementTree, node: NodeId, pred: impl Fn(&NodeKind) -> bool) -> bool {
    let sg = tree.scene_graph();
    let mut current = Some(node);
    while let Some(id) = current {
        if let Some(n) = sg.get(id) {
            if pred(&n.kind) {
                return true;
            }
        }
        current = sg.parent_of(id);
    }
    false
}

#[test]
fn nested_inner_thumb_is_anchored_and_clipped_inside_the_outer_box() {
    // 外側 200×100（収まる）が内側 180×80 の scroll-view を持ち、その 180×300
    // コンテンツが縦にオーバーフローする。サムを描くのは内側のみで、そのサムは
    // 内側 ScrollView アンカー下に吊られる。アンカー自体が外側 ScrollView の Clip
    // 下にネストするため、内側サムは内側ボックスを追従しつつ外側ボックスに制約され、
    // 外へはみ出せない。
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let inner = tree.element_create(2, ElementKind::ScrollView);
    let content = tree.element_create(3, ElementKind::View);
    tree.set_root(outer);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(180.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(180.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, content);
    tree.render(0.0);

    let thumbs = thumb_nodes(&tree);
    assert_eq!(thumbs.len(), 1, "only the overflowing inner scroll-view draws a thumb");
    let inner_thumb = thumbs[0].0;

    assert!(
        is_under_scroll_anchor(&tree, inner_thumb, inner),
        "the inner thumb is lowered under the inner ScrollView anchor",
    );
    assert!(
        is_under_scroll_anchor(&tree, inner_thumb, outer),
        "the inner thumb nests under the outer ScrollView anchor too",
    );
    assert!(
        has_ancestor(&tree, inner_thumb, |k| matches!(k, NodeKind::Clip { .. })),
        "内側サムは外側ボックスでクリップされ外へはみ出せない",
    );
}

#[test]
fn thumb_tracks_the_scroll_offset() {
    let (mut tree, scroll) = vertical_overflow_scroll_view();
    let at_top = thumb_nodes(&tree)[0].2;

    tree.element_set_scroll_offset(scroll, 0.0, 200.0);
    tree.render(0.0);
    let scrolled = thumb_nodes(&tree)[0].2;

    assert!(
        scrolled > at_top,
        "下スクロールでサムがトラックを下る (top {at_top} -> {scrolled})",
    );

    // 最後までスクロールするとサム下端はトラック末尾に達する。形状はスクロール可能
    // 範囲に対するオフセット比に追従する。
    let (_, _, ty, _, th) = thumb_nodes(&tree)[0];
    let (_, sy, _, sh) = tree.element_layout_rect(scroll).unwrap();
    assert!(
        (ty + th) <= sy + sh + 0.01 && (ty + th) >= sy + sh - 4.0,
        "最大オフセットでサムはトラック下端に位置する",
    );
}
