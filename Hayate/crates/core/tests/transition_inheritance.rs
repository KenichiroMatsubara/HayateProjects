//! 祖先の*継承可能*なテキストスタイルの変更が継承を通じて伝播し（ADR-0065 の
//! 2 チャネル: text-local + ambient default）、継承する子孫が自身の連続プロパティを
//! 解決済み `transition-duration` で補間する。`resolve_effective` のプロパティ単位
//! diff（ADR-0093）は、子自身の mutation には現れない非ローカル変更（親の変更で
//! 子の計算値が変わる）を捕捉する。`render(timestamp_ms)` と描画された
//! `DrawTextRun` の色で end-to-end に検証する。

use hayate_core::{
    render_scene_graph, Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree,
    RecordingPainter, StyleProp,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    painter.into_ops()
}

/// 現在の（保持された）シーンで最初に描画されたテキストランの色。
fn text_color(tree: &ElementTree) -> [f32; 4] {
    for op in draw_ops(tree) {
        if let DrawOp::DrawTextRun { color, .. } = op {
            return color;
        }
    }
    panic!("no DrawTextRun in scene");
}

/// シーンで最初に塗られた矩形（親ボックスの背景）の色。
fn background(tree: &ElementTree) -> [f32; 4] {
    for op in draw_ops(tree) {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in scene");
}

/// ambient な `default-color`（ch2, ブロック貫通）を持つ親 `View` と、それを継承
/// する子 `Text`。子が `transition-duration` を持つため、親の default-color が
/// 変わると継承色を補間する。
fn view_over_text(
    default_color: Color,
    child_duration_ms: f32,
) -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::DefaultColor(default_color),
        ],
    );
    tree.element_set_style(text, &[StyleProp::TransitionDuration(child_duration_ms)]);
    tree.element_set_text(text, "Hello");
    (tree, view, text)
}

#[test]
fn inherited_default_color_change_interpolates_descendant_text() {
    // AC1: 親の継承可能プロパティの変更で、子の対応プロパティが目標へ補間される。
    let (mut tree, view, _text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 200.0);
    tree.render(0.0);
    let start = text_color(&tree);
    assert!(
        (start[0] - 1.0).abs() < 1e-3 && start[2].abs() < 1e-3,
        "child text starts red (inherited): {start:?}"
    );

    // *親*を変更する。子自身は変更されず、その計算値だけが変わる。
    tree.element_set_style(
        view,
        &[StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );

    // 変更後の最初のフレームで子のクロックを固定。まだ赤。
    tree.render(100.0);
    let anchored = text_color(&tree);
    assert!(
        (anchored[0] - 1.0).abs() < 1e-3 && anchored[2].abs() < 1e-3,
        "frame 0 of the inherited transition is still red: {anchored:?}"
    );

    // 中間点: 赤と青の間。
    tree.render(200.0);
    let mid = text_color(&tree);
    assert!(mid[0] < 1.0 && mid[0] > 0.0, "red channel mid: {}", mid[0]);
    assert!(mid[2] > 0.0 && mid[2] < 1.0, "blue channel mid: {}", mid[2]);

    // ウィンドウ通過後: 完全に青。
    tree.render(300.0);
    let end = text_color(&tree);
    assert!(
        end[2] > 0.999 && end[0].abs() < 1e-3,
        "child settles on the inherited blue: {end:?}"
    );
}

#[test]
fn descendant_without_duration_takes_inherited_target_immediately() {
    // AC2: 子孫自身の解決済み `transition-duration` が 0 / 未設定なら、継承変更は
    // 目標へ即スナップする。補間も実行中トランジションもなし。
    let (mut tree, view, text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 0.0);
    tree.render(0.0);
    assert!((text_color(&tree)[0] - 1.0).abs() < 1e-3, "starts red");

    tree.element_set_style(
        view,
        &[StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );
    tree.render(100.0);

    let after = text_color(&tree);
    assert!(
        after[2] > 0.999 && after[0].abs() < 1e-3,
        "zero-duration descendant jumps straight to inherited blue: {after:?}"
    );
    assert!(
        !tree.test_transition_active(text),
        "no transition is started for a zero-duration descendant"
    );
}

#[test]
fn ancestor_change_re_evaluates_descendant_so_its_diff_runs() {
    // AC3: 継承変更は非ローカルで、親だけが変わる。プロパティ単位トランジションを
    // 開始させるには子孫を diff に引き戻す（再 emit する）必要がある。開始された
    // トランジションは子の `resolve_effective` diff が走った証拠。以降は収束まで
    // 再評価がスケジュールされ続ける。
    let (mut tree, view, text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 200.0);
    tree.render(0.0);
    assert!(
        !tree.test_transition_active(text),
        "no transition before the ancestor changes"
    );

    tree.element_set_style(
        view,
        &[StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );
    tree.render(100.0);

    assert!(
        tree.test_transition_active(text),
        "the child's diff ran off the ancestor change and started a transition"
    );
    assert!(
        tree.test_visual_dirty_contains(text),
        "an interpolating child stays visual-dirty for continued re-evaluation"
    );
}

#[test]
fn mutated_parent_interpolates_its_own_property_alongside_inherited_child() {
    // AC4: 親は変更点であると同時に継承元でもある。親自身の `background-color` と
    // 渡す ambient `default-color` を 1 回の `setStyle` で変えると、親のボックスと
    // 子のテキストを同時に補間する。継承経路が親の直接変更経路を退行させてはならない。
    let (mut tree, view, text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(200.0),
        ],
    );
    tree.render(0.0);
    assert!(
        (background(&tree)[0] - 1.0).abs() < 1e-3,
        "parent box starts red"
    );
    assert!(
        (text_color(&tree)[0] - 1.0).abs() < 1e-3,
        "child text starts red"
    );

    // 1 回の変更で親自身のボックスと継承する子の両方を駆動する。
    tree.element_set_style(
        view,
        &[
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.render(100.0); // 両方のクロックを固定
    tree.render(200.0); // 中間点

    let parent_mid = background(&tree);
    assert!(
        parent_mid[0] > 0.0 && parent_mid[0] < 1.0 && parent_mid[2] > 0.0 && parent_mid[2] < 1.0,
        "parent box interpolates its own background (#227 intact): {parent_mid:?}"
    );
    assert!(
        tree.test_transition_active(view),
        "parent's own transition is live"
    );

    let child_mid = text_color(&tree);
    assert!(
        child_mid[0] > 0.0 && child_mid[0] < 1.0 && child_mid[2] > 0.0 && child_mid[2] < 1.0,
        "child text interpolates the inherited colour at the same time: {child_mid:?}"
    );
    assert!(
        tree.test_transition_active(text),
        "child's inherited transition is live"
    );
}
