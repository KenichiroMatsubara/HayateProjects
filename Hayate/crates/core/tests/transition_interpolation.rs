//! transition は `resolve_effective` のプロパティ単位 diff で起動する（ADR-0093）。
//! 擬似切り替え・`setStyle`・継承変化はいずれも連続な visual プロパティを
//! `transition-duration` ms かけて補間し、`render(timestamp_ms)` が駆動する。
//! `from` は画面上（ブレンド後）の値なので逆方向の割り込みが連続的に反転し、
//! duration/timing は変更後の解決済み visual から取る。

use hayate_core::{
    render_scene_graph, Color, Dimension, DrawOp, ElementKind, ElementTree, PseudoState,
    RecordingPainter, StyleProp,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    painter.into_ops()
}

/// 現在の（保持された）シーンで最初の塗り rect。
fn first_fill(tree: &ElementTree) -> ([f32; 4], f32) {
    for op in draw_ops(tree) {
        if let DrawOp::FillRect {
            color,
            corner_radius,
            ..
        } = op
        {
            return (color, corner_radius);
        }
    }
    panic!("no FillRect in scene");
}

/// 現在のシーンで最初の塗り rect の背景色。
fn background(tree: &ElementTree) -> [f32; 4] {
    first_fill(tree).0
}

/// 完全な ephemeral 再構築（一致検証の基準経路）が塗る背景色。
fn ephemeral_background(tree: &ElementTree) -> [f32; 4] {
    for op in tree.test_scene_full_rebuild_draw_ops() {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in ephemeral scene");
}

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
fn hover_transition_interpolates_background_color_over_duration() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    assert_eq!(background(&tree)[0], 1.0, "starts fully red");

    tree.update_pointer_hover(Some(root));

    // ホバー後の最初のフレームが transition クロックを固定する。まだ赤。
    tree.render(100.0);
    let start = background(&tree);
    assert!(
        (start[0] - 1.0).abs() < 1e-3 && start[1].abs() < 1e-3,
        "frame 0 still red"
    );

    // 200ms ウィンドウの中間: 赤と緑の間。
    tree.render(200.0);
    let mid = background(&tree);
    assert!(
        mid[0] < 1.0 && mid[0] > 0.0,
        "red channel mid-transition: {}",
        mid[0]
    );
    assert!(
        mid[1] > 0.0 && mid[1] < 1.0,
        "green channel mid-transition: {}",
        mid[1]
    );

    // ウィンドウ通過後: 完全に緑。
    tree.render(300.0);
    let end = background(&tree);
    assert!((end[0]).abs() < 1e-3, "red channel done: {}", end[0]);
    assert!(
        (end[1] - 1.0).abs() < 1e-3,
        "green channel done: {}",
        end[1]
    );
}

#[test]
fn zero_duration_switches_immediately() {
    let (mut tree, root) = hover_box(0.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(0.0);
    let after = background(&tree);
    assert!(
        (after[0]).abs() < 1e-3 && (after[1] - 1.0).abs() < 1e-3,
        "zero-duration hover must switch straight to green: {after:?}"
    );
    assert!(
        !tree.test_transition_active(root),
        "no transition is started"
    );
}

#[test]
fn set_style_interpolates_continuous_property_over_duration() {
    // 直接の `setStyle`（擬似状態切り替えなし）も実効 visual 変化の一種なので、
    // duration > 0 のとき補間する（ADR-0093。Canvas/DOM の意味論一致を回復）。
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    assert_eq!(background(&tree)[0], 1.0, "starts fully red");

    tree.element_set_style(
        root,
        &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );

    // 変更後の最初のフレームがクロックを固定する。まだ赤。
    tree.render(100.0);
    let start = background(&tree);
    assert!(
        (start[0] - 1.0).abs() < 1e-3 && start[2].abs() < 1e-3,
        "setStyle frame 0 still red: {start:?}"
    );
    assert!(
        tree.test_transition_active(root),
        "setStyle starts a transition"
    );

    // ウィンドウ中間: 赤と青の間。
    tree.render(200.0);
    let mid = background(&tree);
    assert!(mid[0] < 1.0 && mid[0] > 0.0, "red mid: {}", mid[0]);
    assert!(mid[2] > 0.0 && mid[2] < 1.0, "blue mid: {}", mid[2]);

    // ウィンドウ通過後: 完全に青。
    tree.render(300.0);
    let end = background(&tree);
    assert!(
        end[2] > 0.999 && end[0].abs() < 1e-3,
        "setStyle settles on blue: {end:?}"
    );
}

#[test]
fn reverse_interrupt_continues_from_displayed_value() {
    // transition の途中で反転すると現在の画面上の値から再開し、解決済みの値へ
    // 飛ばない。
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // 固定
    tree.render(200.0); // 赤 -> 緑 の中間
    let mid = background(&tree);
    assert!(
        mid[0] > 0.0 && mid[1] > 0.0,
        "captured a mid value: {mid:?}"
    );

    // 同じ瞬間に反転: 表示値が飛んではならない。
    tree.update_pointer_hover(None);
    tree.render(200.0);
    let reversed = background(&tree);
    assert!(
        (reversed[0] - mid[0]).abs() < 1e-2 && (reversed[1] - mid[1]).abs() < 1e-2,
        "reversal is continuous, not a jump: {mid:?} -> {reversed:?}"
    );

    // 反転を続けると赤へ戻る（赤チャンネルが上昇する）。
    tree.render(300.0);
    let back = background(&tree);
    assert!(
        back[0] > reversed[0],
        "red channel climbs back: {} -> {}",
        reversed[0],
        back[0]
    );
}

#[test]
fn duration_is_read_from_after_change_resolved_visual() {
    // base 500ms に対する `:hover { transition-duration: 0 }` は hover-in を即時、
    // hover-out をアニメーションにする。duration は変更後の解決済み値であり、
    // base の直接読みではない。
    let mut tree = ElementTree::new();
    let root = tree.element_create(2, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(500.0),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(0.0),
        ],
    );
    tree.render(0.0);

    // hover-in: 変更後 duration が 0 → 即座に緑、transition なし。
    tree.update_pointer_hover(Some(root));
    tree.render(100.0);
    let hovered = background(&tree);
    assert!(
        (hovered[1] - 1.0).abs() < 1e-3 && hovered[0].abs() < 1e-3,
        "hover-in is instant: {hovered:?}"
    );
    assert!(
        !tree.test_transition_active(root),
        "instant hover-in starts no transition"
    );

    // hover-out: 変更後 duration が base 500ms → 赤へアニメーションで戻る。
    tree.update_pointer_hover(None);
    tree.render(200.0); // 固定
    tree.render(450.0); // 500ms ウィンドウの 250ms 地点
    let mid = background(&tree);
    assert!(
        mid[0] > 0.0 && mid[0] < 1.0 && mid[1] > 0.0 && mid[1] < 1.0,
        "hover-out animates over 500ms: {mid:?}"
    );
    assert!(
        tree.test_transition_active(root),
        "hover-out starts a transition"
    );
}

#[test]
fn first_emit_shows_target_without_transition() {
    // 要素の最初のレンダリングは即座にターゲットを取る。補間元となる
    // 変更前の値が存在しないため。
    let (mut tree, root) = hover_box(200.0);
    // 初回レンダリング前から既にホバーが有効。
    tree.update_pointer_hover(Some(root));
    tree.render(0.0);
    let first = background(&tree);
    assert!(
        (first[1] - 1.0).abs() < 1e-3 && first[0].abs() < 1e-3,
        "first emit paints the target (green) with no interpolation: {first:?}"
    );
    assert!(
        !tree.test_transition_active(root),
        "first emit starts no transition"
    );
}

#[test]
fn properties_interpolate_from_independent_from_values() {
    // 要素 × プロパティ単位の状態: 後から変わるプロパティは自身の値と開始時刻から
    // 始まり、先行するプロパティは走り続ける。
    let mut tree = ElementTree::new();
    let root = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::BorderRadius(0.0),
            StyleProp::TransitionDuration(200.0),
        ],
    );
    tree.render(0.0);

    // 背景は t=100 で変化を開始する。
    tree.element_set_style(
        root,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.render(100.0);

    // border-radius は 1 フレーム遅れて t=200 で変化を開始する。
    tree.element_set_style(root, &[StyleProp::BorderRadius(20.0)]);
    tree.render(200.0);

    // t=300 では背景（100 開始）は 200ms ウィンドウを完了し、radius（200 開始）は
    // まだ半分。クロックは独立している。
    tree.render(300.0);
    let (color, radius) = first_fill(&tree);
    assert!(
        (color[1] - 1.0).abs() < 1e-3 && color[0].abs() < 1e-3,
        "background finished its own window: {color:?}"
    );
    assert!(
        radius > 0.0 && radius < 20.0,
        "border-radius is mid-flight on its own clock: {radius}"
    );
}

#[test]
fn full_ephemeral_rebuild_paints_target_without_interpolation() {
    // ephemeral（一致検証の基準）経路は保持された `last_displayed` を持たないので
    // 補間せず、解決済みのターゲットを塗る。
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // 固定
    tree.render(200.0); // 保持経路は transition の途中

    let retained = background(&tree);
    assert!(
        retained[0] > 0.0 && retained[1] > 0.0 && retained[1] < 1.0,
        "retained path is mid-transition: {retained:?}"
    );

    let ephemeral = ephemeral_background(&tree);
    assert!(
        (ephemeral[1] - 1.0).abs() < 1e-3 && ephemeral[0].abs() < 1e-3,
        "ephemeral rebuild paints the resolved target (green): {ephemeral:?}"
    );
}
