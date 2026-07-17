//! 読み取り専用クエリ `element_displayed_visual(id, now_ms)` は、進行中の
//! トランジション値を retained state から直接観測する（`render()`→SceneGraph
//! 走査やフレームループを介さない）。render パスと同一の補間（ADR-0093）だが
//! `&self` で副作用がなく、render のメモ化済みトランジション状態を進めない。

use hayate_core::{
    render_scene_graph, Color, Dimension, DrawOp, ElementKind, ElementTree, PseudoState,
    RecordingPainter, StyleProp,
};

/// retained scene が描く最初の塗りつぶし矩形の背景色。
fn painted_background(tree: &ElementTree) -> [f32; 4] {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    for op in painter.into_ops() {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in scene");
}

/// 静止時は赤、`:hover` で `duration_ms` かけて緑へ遷移する 100×50 のボックス。
/// `transition_interpolation.rs` のフィクスチャと同一。
fn hover_box(duration_ms: f32) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(duration_ms),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    (tree, root)
}

#[test]
fn displayed_visual_observes_mid_transition_without_a_frame_at_that_time() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // トランジション開始時刻を t=100 に固定。まだ赤

    // t=200 で render せずに 200ms ウィンドウの中間を問い合わせる。進行中の
    // トラックは retained state から直接サンプリングされる。
    let mid = tree
        .element_displayed_visual(root, 200.0)
        .unwrap()
        .background_color
        .unwrap();
    assert!(
        mid.r < 1.0 && mid.r > 0.0,
        "red channel is mid-transition: {}",
        mid.r
    );
    assert!(
        mid.g > 0.0 && mid.g < 1.0,
        "green channel is mid-transition: {}",
        mid.g
    );
}

#[test]
fn displayed_visual_returns_effective_target_when_settled() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);

    // 進行中のものが無ければ、displayed visual は解決済みターゲットそのもの。
    let displayed = tree.element_displayed_visual(root, 0.0).unwrap();
    assert_eq!(
        displayed.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "a settled element's displayed visual is its effective target"
    );
}

#[test]
fn displayed_visual_does_not_advance_render_state() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // t=100 に固定

    // ウィンドウ終端をはるかに過ぎた時刻でのクエリは、render パスのように
    // retained state を変えるならトラックを緑に確定して破棄してしまう。
    // クエリは進行中のトランジションに手を触れてはならない。
    let _ = tree.element_displayed_visual(root, 100_000.0);
    assert!(
        tree.test_transition_active(root),
        "query must not complete or drop the in-flight transition"
    );

    // よって次の実フレームは render が止めた地点（t=100）から進み、緑へ
    // スナップせず t=200 でウィンドウ中間に着く。
    tree.render(200.0);
    let painted = painted_background(&tree);
    assert!(
        painted[0] > 0.0 && painted[1] > 0.0 && painted[1] < 1.0,
        "render continues mid-transition, unperturbed by the query: {painted:?}"
    );
}

#[test]
fn displayed_visual_matches_what_render_paints_at_the_same_time() {
    // クエリと render パスは単一の補間を共有しなければならない（実装の二重化
    // 禁止）。同じ retained state に同じブレンドを走らせるため、`now_ms` での
    // サンプリングは `now_ms` のフレームが描く結果と一致する。
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // t=100 に固定

    // まずクエリで観測し（副作用なし）、次に同時刻でフレームを描く。
    // 両者は t=100 から同じトラックを進める。
    let queried = tree
        .element_displayed_visual(root, 175.0)
        .unwrap()
        .background_color
        .unwrap();
    tree.render(175.0);
    let painted = painted_background(&tree);

    assert!(
        (queried.r as f32 - painted[0]).abs() < 1e-4
            && (queried.g as f32 - painted[1]).abs() < 1e-4
            && (queried.b as f32 - painted[2]).abs() < 1e-4,
        "query must agree with the painted frame at the same time: \
         query={queried:?} painted={painted:?}"
    );
}
