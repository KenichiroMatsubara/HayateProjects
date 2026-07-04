//! compositing layer ツリー＋`layer_dirty` のコア統合（ADR-0125 コア半分・#609）。
//!
//! 公開インターフェース（`ElementTree`）越しに、compositing trigger（transform group / scroll
//! コンテナ）からレイヤ境界が自動判定され、レイヤ id が境界要素の `ElementId` に一致し、要素 dirty が
//! 内包する最近接レイヤへ `layer_dirty` として流れることを固定する。純粋な境界判定/導出ロジック自体は
//! `element::compositing` の単体テストにある（ElementTree 非依存）。

use std::collections::HashSet;

use hayate_core::{Color, ElementKind, ElementTree};
use hayate_core::element::style::{Dimension, StyleProp};

#[test]
fn scroll_view_and_transform_elements_become_layers() {
    // root(view) > scroll(ScrollView) > item(view); root > boxed(view, transform)
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let item = tree.element_create(2, ElementKind::View);
    let boxed = tree.element_create(3, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, item);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 10.0, 0.0]));

    let layers = tree.load_compositing_layers();
    let set: HashSet<_> = layers.layers.iter().copied().collect();

    // compositing trigger（scroll コンテナ / transform group）を持つ要素だけがレイヤ境界。
    assert!(set.contains(&scroll), "ScrollView は compositing layer になる");
    assert!(set.contains(&boxed), "transform 要素は compositing layer になる");
    assert!(!set.contains(&root), "通常 view（root）はレイヤでない");
    assert!(!set.contains(&item), "通常 view（item）はレイヤでない");

    // レイヤ id ＝境界要素の ElementId。どちらも root 直下で root は非レイヤ＝親レイヤ無し。
    assert_eq!(layers.parent.get(&scroll), Some(&None));
    assert_eq!(layers.parent.get(&boxed), Some(&None));
}

#[test]
fn nested_layer_parent_is_the_enclosing_scroll_layer() {
    // root(view) > scroll(ScrollView) > moving(view, transform)
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let moving = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, moving);
    tree.set_root(root);
    tree.element_set_transform(moving, Some([1.0, 0.0, 0.0, 1.0, 5.0, 0.0]));

    let layers = tree.load_compositing_layers();
    // scroll はルートレイヤ、moving の親レイヤは内包する scroll。
    assert_eq!(layers.parent.get(&scroll), Some(&None));
    assert_eq!(layers.parent.get(&moving), Some(&Some(scroll)));
}

// ── #632: `render()` 内捕捉の frame_layers / frame_layer_dirty（root 暗黙レイヤ）──────────
//
// present 側の raster gating は「このフレームで scene が実際に変わったか」を必要とする。
// カーソル点滅・スクロール慣性・インジケータ fade は `render()` の冒頭でマークされ**同フレーム内で
// drain** されるため、render 前の `layer_dirty()` スナップショットでは取りこぼす。そこで `render()` が
// scene_build に dirty を渡す瞬間に捕捉した集合を `frame_layer_dirty()` として公開する。また、どの
// trigger レイヤにも内包されない dirty を落とさないよう、root を暗黙の compositing layer 境界として
// `frame_layers()` に必ず含める（Blink の root layer と同型）。

#[test]
fn frame_capture_includes_root_as_implicit_layer() {
    // root(view) > child(view)（trigger なし）でも、捕捉レイヤ列は root を暗黙境界として含む。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let child = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, child);
    tree.set_root(root);

    let _ = tree.render(0.0);
    assert_eq!(
        tree.frame_layers().first(),
        Some(&root),
        "root は暗黙の compositing layer（描画順の先頭）"
    );
    // 初回は全面構築＝root レイヤが dirty（cold cache と同じ扱いで全面 raster される）。
    assert!(tree.frame_layer_dirty().contains(&root));
}

#[test]
fn clean_frame_captures_empty_layer_dirty() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    tree.set_root(root);

    let _ = tree.render(0.0);
    let _ = tree.render(16.0);
    assert!(
        tree.frame_layer_dirty().is_empty(),
        "変化のないフレームの捕捉 dirty は空（raster を呼ばない前提）"
    );
    assert_eq!(tree.frame_layers().first(), Some(&root), "clean フレームでもレイヤ列は保持");
}

#[test]
fn out_of_layer_dirty_routes_to_root_layer() {
    // trigger レイヤに内包されない dirty は `layer_dirty()` では無視されるが、frame 捕捉では
    // root 暗黙レイヤへ流れる（取りこぼすと raster がスキップされ stale frame になる）。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let child = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, child);
    tree.set_root(root);

    let _ = tree.render(0.0);
    tree.element_set_style(child, &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))]);
    let _ = tree.render(16.0);
    assert!(
        tree.frame_layer_dirty().contains(&root),
        "レイヤ外の dirty は root 暗黙レイヤとして捕捉される"
    );
}

#[test]
fn dirty_inside_scroll_layer_is_captured_on_the_scroll_layer_not_root() {
    // root(view) > scroll(ScrollView) > item(view)。item の変化は内包する scroll レイヤに畳まれ、
    // root（他レイヤ）は clean のまま＝damage 比例の再 raster 前提を core 側で固定する。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let item = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, item);
    tree.set_root(root);

    let _ = tree.render(0.0);
    tree.element_set_style(item, &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))]);
    let _ = tree.render(16.0);
    let dirty = tree.frame_layer_dirty();
    assert!(dirty.contains(&scroll), "item の dirty は内包する scroll レイヤへ");
    assert!(!dirty.contains(&root), "他レイヤ（root）は clean のまま");
    assert!(tree.frame_layers().contains(&scroll), "scroll はレイヤ列に含まれる");
}

#[test]
fn in_render_transition_continuation_is_captured() {
    // 進行中 transition は render 後に re-mark され、次フレームの lowering 集合として捕捉される。
    // render 前スナップショット方式だと 2 フレーム目以降の補間が取りこぼされる回帰をここで防ぐ。
    use hayate_core::element::style::Dimension;
    use hayate_core::PseudoState;

    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(200.0),
        ],
    );
    tree.element_set_pseudo_style(
        boxed,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );

    let _ = tree.render(0.0);
    tree.update_pointer_hover(Some(boxed));
    let _ = tree.render(16.0); // transition 開始
    // 以後の補間フレーム：外部からのマークは無いが、進行中 transition が捕捉され続ける。
    let _ = tree.render(32.0);
    assert!(
        !tree.frame_layer_dirty().is_empty(),
        "補間中フレームの捕捉 dirty は空でない（stale frame を防ぐ）"
    );
}

// ── #680: レイヤ降格（transition 終了）は内包する親レイヤを dirty にする ────────────────
//
// タップ等で一時的に compositing layer へ昇格した兄弟要素（active transition trigger）が、
// 昇格中に他の視覚変化で親（root）レイヤが「その兄弟を除外した」raster を一度でも行うと、
// 親レイヤのキャッシュ texture はその兄弟の領域を穴あきのまま持つ（昇格中は兄弟自身の
// texture がその穴を埋めるので見た目には出ない）。transition が終わって兄弟がレイヤ境界
// から外れる（親へ畳み戻る）と、兄弟自身の texture は破棄され、親レイヤは dirty でない
// 限り穴あきキャッシュのまま再利用される＝兄弟要素が画面から消えたまま戻らない回帰。

#[test]
fn layer_demotion_marks_the_enclosing_layer_dirty() {
    use hayate_core::element::style::Dimension;
    use hayate_core::PseudoState;

    // root(view) > sibling(view, hover で transition), other(view, 無関係の兄弟)。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let sibling = tree.element_create(1, ElementKind::View);
    let other = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, sibling);
    tree.element_append_child(root, other);
    tree.set_root(root);
    tree.element_set_style(
        sibling,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(200.0),
        ],
    );
    tree.element_set_pseudo_style(
        sibling,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );

    let _ = tree.render(0.0);
    tree.update_pointer_hover(Some(sibling)); // タップ相当：sibling に transition を起こす
    let _ = tree.render(16.0); // transition 開始（is_active は次フレームまで未反映）

    // 昇格中に他の兄弟（other）の視覚変化で root を再 raster させる（cursor blink 等の実際の
    // トリガを模す）。root は sibling を除外した「穴あき」キャッシュを持つことになる。
    tree.element_set_style(other, &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))]);
    let _ = tree.render(200.0);
    assert!(
        tree.frame_layers().contains(&sibling),
        "transition 中は sibling が独立 compositing layer"
    );
    assert!(
        tree.frame_layer_dirty().contains(&root),
        "root は sibling を除外した穴あき raster を行う（前提の確認）"
    );

    // transition が完了する時刻（start_ms=16.0 + duration 200.0）まで進める。
    let _ = tree.render(216.0);
    // 降格フレーム：sibling はもう独立レイヤでない。他に何も dirty がなくても、
    // 畳み戻り先の root は穴あきキャッシュを直すため content dirty にならなければならない。
    let _ = tree.render(232.0);
    assert!(
        !tree.frame_layers().contains(&sibling),
        "transition 終了で sibling はレイヤから降格し root へ畳み戻る"
    );
    assert!(
        tree.frame_layer_dirty().contains(&root),
        "降格した sibling を内包する root が content dirty にならないと、穴あきキャッシュが\
         再利用され続け sibling が画面から消えたままになる（#680 回帰）"
    );
}

#[test]
fn on_demand_loop_keeps_rendering_through_layer_demotion() {
    // #680 実機回帰：Android の on-demand フレームループは `has_pending_visual_work()` が
    // false になった時点で render() を止める（ADR-0117/0126）。だが is_active() は
    // `capture_frame_layers`（render 冒頭）より後で `scene_build::update` が更新するため、
    // transition が完了する render() 呼び出しの**その場では**まだ降格（frame_layers からの
    // 除外）が反映されない——反映されるのは次の render() 呼び出し（1 フレーム遅延・上の
    // テスト参照）。`has_pending_visual_work` が「transition がまだ active か」だけを見て
    // いると、is_active が false に落ちた瞬間にループが停止し、降格を capture する
    // render() がもう二度と呼ばれない。昇格中に穴あき raster された親レイヤのキャッシュが
    // 永久に再利用され続け、実機では「タップした瞬間に消えたまま」固まる（別の場所を
    // もう一度タップして初めて次の render() が起き、直る——ユーザ報告の「謎挙動」の正体）。
    use hayate_core::element::style::Dimension;
    use hayate_core::PseudoState;

    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let sibling = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, sibling);
    tree.set_root(root);
    tree.element_set_style(
        sibling,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(20.0),
        ],
    );
    tree.element_set_pseudo_style(
        sibling,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );

    let _ = tree.render(0.0);
    tree.update_pointer_hover(Some(sibling)); // タップ相当
    assert!(tree.has_pending_visual_work(), "hover 直後は継続フレームを要求する");

    // on-demand ループそのものを模す：継続要求がある間だけ render を回す。
    let mut t = 16.0;
    let mut frames = 0;
    while tree.has_pending_visual_work() {
        let _ = tree.render(t);
        t += 16.0;
        frames += 1;
        assert!(frames < 200, "有限フレームで idle に落ちなければならない");
    }

    assert!(
        !tree.frame_layers().contains(&sibling),
        "ループが idle に落ちた時点で sibling の降格が capture_frame_layers に反映されて\
         いなければならない。さもなくば穴あきキャッシュのまま二度と render() が呼ばれず、\
         画面が消えたまま固まる（#680 実機回帰の芯）"
    );
}

#[test]
fn on_demand_loop_settles_two_concurrent_sibling_transitions() {
    // #680 実機回帰（追加ケース）: 実機の優先度セグメントボタン（高/中/低）は 1 回の
    // クリックで「選択されていた方」が非アクティブ色へ、「新しく選択された方」がアクティブ色へ、
    // 2 要素が同一フレームで同時に transition を開始し、同じ duration で同時に終わる
    // （`AddForm.tsx` の `seg()` — `backgroundColor: active ? tone : panel2` を 2 兄弟へ同時適用）。
    // 既存の回帰テストは 1 要素だけが transition する場合しか固定していない。2 要素が同時に
    // 昇格・降格する場合、`has_pending_layer_transition` の境界差分比較は集合演算なので原理上
    // 複数要素でも成立するはずだが、実機では両方降格せず片方（または両方）が消えたまま固まる
    // 挙動が観測されている——ここでその前提を明示的に固定する。
    use hayate_core::element::style::Dimension;

    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let a = tree.element_create(1, ElementKind::View); // 元々アクティブ（緑）→ 非アクティブへ
    let b = tree.element_create(2, ElementKind::View); // 元々非アクティブ（灰）→ アクティブへ
    tree.element_append_child(root, a);
    tree.element_append_child(root, b);
    tree.set_root(root);

    let active = Color::new(0.0, 1.0, 0.0, 1.0);
    let inactive = Color::new(0.5, 0.5, 0.5, 1.0);
    let common = |bg: Color| {
        vec![
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(bg),
            StyleProp::TransitionDuration(160.0),
        ]
    };
    tree.element_set_style(a, &common(active));
    tree.element_set_style(b, &common(inactive));
    let _ = tree.render(0.0);

    // タップ相当：クリックハンドラが同一フレームで a を非アクティブへ、b をアクティブへ
    // 同時に切り替える（`onClick={() => props.onPrio(prio)}` の再描画で両方の `seg()` が
    // 新しい `active` 値で呼ばれるのと同型）。
    tree.element_set_style(a, &[StyleProp::BackgroundColor(inactive)]);
    tree.element_set_style(b, &[StyleProp::BackgroundColor(active)]);
    assert!(tree.has_pending_visual_work(), "スタイル変更直後は継続フレームを要求する");

    let mut t = 16.0;
    let mut frames = 0;
    while tree.has_pending_visual_work() {
        let _ = tree.render(t);
        t += 16.0;
        frames += 1;
        assert!(frames < 200, "有限フレームで idle に落ちなければならない");
    }

    assert!(
        !tree.frame_layers().contains(&a),
        "idle 到達時点で a の降格が capture_frame_layers に反映されていなければならない\
         （2 要素同時 transition の片方が取り残される #680 回帰）"
    );
    assert!(
        !tree.frame_layers().contains(&b),
        "idle 到達時点で b の降格も同様に反映されていなければならない"
    );
    assert!(
        tree.frame_layer_dirty().contains(&root),
        "a・b の両方を内包する root は穴あきキャッシュを直すため content dirty にならなければ\
         ならない（片方だけ直って他方が消えたままになる回帰を防ぐ）"
    );
}

#[test]
fn on_demand_loop_settles_sibling_transitions_inside_a_scroll_view() {
    // #680 実機回帰（追加ケース・2）: 実機の優先度セグメントボタンは root 直下ではなく、
    // Tasks パネル全体を包む ScrollView の中にある（`AddForm` は Tasks カードの子）。
    // ScrollView 自身が常時 compositing layer 境界なので、ボタンの降格 dirty は root では
    // なく「内包する ScrollView」へ流れる（`derive_layer_dirty` は最近接レイヤへルーティング
    // する）。上のテストは root 直下の平だな階層でしか固定していないので、ScrollView を
    // 挟んだ経路に別の穴（demoted dirty が ScrollView の chrome/content 分離を通り抜けられ
    // ない等）が無いかをここで固定する。
    use hayate_core::element::style::Dimension;

    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let a = tree.element_create(2, ElementKind::View);
    let b = tree.element_create(3, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, a);
    tree.element_append_child(scroll, b);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(100.0))],
    );

    let active = Color::new(0.0, 1.0, 0.0, 1.0);
    let inactive = Color::new(0.5, 0.5, 0.5, 1.0);
    let common = |bg: Color| {
        vec![
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(bg),
            StyleProp::TransitionDuration(160.0),
        ]
    };
    tree.element_set_style(a, &common(active));
    tree.element_set_style(b, &common(inactive));
    let _ = tree.render(0.0);

    tree.element_set_style(a, &[StyleProp::BackgroundColor(inactive)]);
    tree.element_set_style(b, &[StyleProp::BackgroundColor(active)]);
    assert!(tree.has_pending_visual_work(), "スタイル変更直後は継続フレームを要求する");

    let mut t = 16.0;
    let mut frames = 0;
    while tree.has_pending_visual_work() {
        let _ = tree.render(t);
        t += 16.0;
        frames += 1;
        assert!(frames < 200, "有限フレームで idle に落ちなければならない");
    }

    assert!(!tree.frame_layers().contains(&a), "idle 到達時点で a は降格済みのはず");
    assert!(!tree.frame_layers().contains(&b), "idle 到達時点で b も降格済みのはず");
    assert!(
        tree.frame_layer_dirty().contains(&scroll),
        "a・b を内包する ScrollView が穴あきキャッシュを直すため content dirty に\
         ならなければならない（root 直下では無く ScrollView 内にある実機構成の回帰）"
    );
}

// ── #633: transform-only 変化は content dirty にしない（composite-only フレームの core 前提）──
//
// `element_set_transform` の Some→Some（係数だけの変化）はレイヤ内容を変えない——変わるのは合成時の
// quad transform だけ。これを visual dirty に流すと transform アニメーションの毎フレームがレイヤ
// 再 raster になり、#633 の「transform だけが変わるフレームは vello raster ゼロ」が成立しない。
// core は係数変化を専用チャネルで受け、保持シーンの Group ノードだけを patch する（re-lower なし）。

#[test]
fn transform_coefficient_change_is_not_content_dirty() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 10.0, 0.0]));
    let _ = tree.render(0.0);

    // Some→Some の係数変化：レイヤ内容は不変なので content dirty（frame_layer_dirty）は空。
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 20.0, 0.0]));
    let _ = tree.render(16.0);
    assert!(
        tree.frame_layer_dirty().is_empty(),
        "transform 係数だけの変化は content dirty にならない（composite-only 前提）"
    );
    // quad 用の現在値は公開 getter から読める。
    assert_eq!(tree.element_transform(boxed), Some([1.0, 0.0, 0.0, 1.0, 20.0, 0.0]));
}

#[test]
fn transform_coefficient_change_still_updates_the_retained_scene() {
    // 全面 raster 経路（FramePlan が raster を選んだフレーム）でも出力が正しいよう、保持シーンの
    // Group ノードは patch されて新しい係数を持つ（re-lower はしない）。
    use hayate_core::{render_scene_graph, DrawOp, RecordingPainter};

    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 10.0, 0.0]));
    let _ = tree.render(0.0);

    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 20.0, 0.0]));
    let _ = tree.render(16.0);

    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    let has_new = painter.ops().iter().any(|op| {
        matches!(op, DrawOp::PushTransform { transform } if *transform == [1.0, 0.0, 0.0, 1.0, 20.0, 0.0])
    });
    assert!(has_new, "保持シーンの Group は新しい transform 係数へ patch される");
}

#[test]
fn transform_none_to_some_is_content_dirty() {
    // None↔Some は emit されるノード構造が変わる（Group ラッパの出現/消滅）＝re-lower が要る。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    let _ = tree.render(0.0);

    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 10.0, 0.0]));
    let _ = tree.render(16.0);
    assert!(
        tree.frame_layer_dirty().contains(&boxed),
        "None→Some は新レイヤの内容構築＝content dirty"
    );

    tree.element_set_transform(boxed, None);
    let _ = tree.render(32.0);
    assert!(
        !tree.frame_layer_dirty().is_empty(),
        "Some→None はラッパ消滅＝content dirty（内包レイヤへ流れる）"
    );
}

#[test]
fn layer_dirty_routes_descendant_dirty_to_enclosing_layer() {
    // root(view) > scroll(ScrollView) > item(view)
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let item = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, item);
    tree.set_root(root);

    // 初期構築の dirty を render で排出してから、item だけを visual-dirty にする。
    let _ = tree.render(0.0);
    tree.element_set_style(item, &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))]);

    // item 自身はレイヤでないが、内包する scroll レイヤが再 raster 対象になる。
    let dirty = tree.layer_dirty();
    assert!(dirty.contains(&scroll), "item の dirty は内包する scroll レイヤへ流れる");
    assert!(!dirty.contains(&item), "layer_dirty はレイヤ id（境界要素）だけを含む");
}

// ── #634: scroll frame は chrome dirty（content 非 dirty＝composite-only スクロール前提）─────
//
// scroll offset だけの変化は content band texture のピクセルを変えない（offset は scroll Group
// affine ＝ composite 段で適用）。変わるのは chrome（スクロールバー/インジケータ、Clip の外側）
// だけなので、`frame_layer_dirty`（content）でなく `frame_layer_chrome_dirty` に流れる。
// 同フレームに他の視覚変化が重なったら chrome-only 判定は毒され、保守的に content dirty へ戻る。

fn scroll_fixture() -> (ElementTree, hayate_core::ElementId, hayate_core::ElementId) {
    // root(view) > scroll(ScrollView) > item(view)
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let item = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, item);
    tree.set_root(root);
    (tree, scroll, item)
}

#[test]
fn scroll_offset_change_is_chrome_dirty_not_content_dirty() {
    let (mut tree, scroll, _) = scroll_fixture();
    let _ = tree.render(0.0);

    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    let _ = tree.render(16.0);
    assert!(
        tree.frame_layer_dirty().is_empty(),
        "offset だけの変化は content dirty にならない（composite-only スクロール前提）"
    );
    assert!(
        tree.frame_layer_chrome_dirty().contains(&scroll),
        "offset 変化は chrome dirty（スクロールバー面の再 raster だけが要る）"
    );
}

#[test]
fn scroll_offset_change_still_updates_the_retained_scene() {
    // chrome dirty の SelfOnly 再 lowering が scroll Group affine を新 offset で emit し直す。
    // composite の quad transform／全面 raster 経路の両方がこれを読むので出力は常に正しい。
    use hayate_core::{render_scene_graph, DrawOp, RecordingPainter};

    let (mut tree, scroll, _) = scroll_fixture();
    let _ = tree.render(0.0);

    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    let _ = tree.render(16.0);

    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    let has_new = painter.ops().iter().any(|op| {
        matches!(op, DrawOp::PushTransform { transform } if *transform == [1.0, 0.0, 0.0, 1.0, 0.0, -50.0])
    });
    assert!(has_new, "保持シーンの scroll Group は新 offset の affine を持つ");
}

#[test]
fn scroll_offset_plus_other_visual_change_is_content_dirty() {
    // 同フレームで背景色も変わったら、SelfOnly 再 lowering は content band 内のピクセル
    //（ScrollView 自身の背景は Clip の内側）も変える＝chrome-only 判定は両順序で毒される。
    for style_first in [true, false] {
        let (mut tree, scroll, _) = scroll_fixture();
        let _ = tree.render(0.0);

        if style_first {
            tree.element_set_style(scroll, &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))]);
            tree.element_set_scroll_offset(scroll, 0.0, 50.0);
        } else {
            tree.element_set_scroll_offset(scroll, 0.0, 50.0);
            tree.element_set_style(scroll, &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))]);
        }
        let _ = tree.render(16.0);
        assert!(
            tree.frame_layer_dirty().contains(&scroll),
            "他の視覚変化が重なったフレームは content dirty（style_first={style_first}）"
        );
        assert!(
            !tree.frame_layer_chrome_dirty().contains(&scroll),
            "content dirty へ昇格したら chrome dirty には残らない（style_first={style_first}）"
        );
    }
}

#[test]
fn descendant_change_during_scroll_keeps_content_dirty() {
    // item の変化は内包レイヤ（scroll）の content dirty。同フレームの offset 変化はそれを消さない。
    let (mut tree, scroll, item) = scroll_fixture();
    let _ = tree.render(0.0);

    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    tree.element_set_style(item, &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))]);
    let _ = tree.render(16.0);
    assert!(
        tree.frame_layer_dirty().contains(&scroll),
        "子孫の変化は offset 変化と同フレームでも content dirty のまま"
    );
}

#[test]
fn touch_indicator_fade_frames_stay_chrome_dirty() {
    // Touch スクロールの一時インジケータは表示→fade の間、毎フレーム SelfOnly 再 lowering される
    //（ADR-0110）。この継続フレームも chrome dirty ＝ content band は composite-only のまま。
    use hayate_core::element::pointer::PointerKind;

    let (mut tree, scroll, _) = scroll_fixture();
    let _ = tree.render(0.0);

    tree.on_pointer_down_with_kind(0.0, 0.0, 0, PointerKind::Touch);
    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    let _ = tree.render(16.0);
    // インジケータ稼働中の継続フレーム（offset は変えない）。
    let _ = tree.render(32.0);
    assert!(
        tree.frame_layer_dirty().is_empty(),
        "インジケータ fade の継続フレームは content dirty にならない"
    );
    assert!(
        tree.frame_layer_chrome_dirty().contains(&scroll),
        "インジケータ fade の継続フレームは chrome dirty"
    );
}

// ── #639: overscroll（rubber バウンド）中も content 非 dirty（composite-only）───────────────
//
// バウンス中に変わるのは scroll Group affine（越境変位込み・rubber-band translate / Android stretch）
// 1 本だけで、content band texture のピクセルは不変。したがって offset が `[0, max]` の外へ出るフレーム
// でも、#634 と同じく content dirty（`frame_layer_dirty`）ではなく chrome dirty へ流れる（合成 transform
// 変更として扱う）。ここは classification（純関数 dirty 分類）を ElementTree 越しに固定する。

fn scrollable_fixture() -> (ElementTree, hayate_core::ElementId) {
    // root > scroll(200x200) > content(200x5000)：max_y > 0 で縦バウンスが成立する。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(200.0))],
    );
    tree.element_set_style(
        content,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(5000.0))],
    );
    (tree, scroll)
}

#[test]
fn overscroll_offset_is_chrome_dirty_not_content_dirty() {
    let (mut tree, scroll) = scrollable_fixture();
    let _ = tree.render(0.0);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    assert!(max_y > 0.0);

    // 下端を越えたバウンス位置（offset > max）。
    tree.element_set_scroll_offset(scroll, 0.0, max_y + 120.0);
    let _ = tree.render(16.0);
    assert!(
        tree.frame_layer_dirty().is_empty(),
        "越境 offset でも content dirty にならない（overshoot は合成 affine の担当・composite-only）"
    );
    assert!(
        tree.frame_layer_chrome_dirty().contains(&scroll),
        "越境 offset の変化も chrome dirty（合成 transform 変更）"
    );

    // 上端を越えたバウンス位置（offset < 0）でも同じ。
    tree.element_set_scroll_offset(scroll, 0.0, -80.0);
    let _ = tree.render(32.0);
    assert!(
        tree.frame_layer_dirty().is_empty(),
        "上端越境 offset でも content dirty にならない"
    );
    assert!(tree.frame_layer_chrome_dirty().contains(&scroll));
}

#[test]
fn overscroll_shows_up_in_scroll_group_affine_per_profile() {
    // 越境変位は content の再 raster ではなく scroll Group affine に現れる。プロファイル差は
    // この 1 本に閉じる（ADR-0131）：iOS は overshoot 込みの素の translate、Android は端ピンの
    // 一様 stretch scale。どちらも present 側 quad transform がそのまま読む（#634 getter）。
    use hayate_core::scroll::ScrollPhysicsProfile;

    // iOS 既定（Auto→iOS）：overshoot 込みで丸ごと translate、scale は恒等。
    let (mut tree, scroll) = scrollable_fixture();
    let _ = tree.render(0.0);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    tree.element_set_scroll_offset(scroll, 0.0, max_y + 120.0);
    let ios = tree.element_scroll_group_affine(scroll);
    assert_eq!(ios, [1.0, 0.0, 0.0, 1.0, 0.0, -(max_y + 120.0) as f64], "iOS は overshoot 込みの素の translate");

    // Android stretch：越境軸は scale > 1 の一様スケール（端ピン）。translate だけでは表せない。
    let (mut tree, scroll) = scrollable_fixture();
    tree.set_scroll_profile(ScrollPhysicsProfile::Android);
    let _ = tree.render(0.0);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    tree.element_set_scroll_offset(scroll, 0.0, max_y + 120.0);
    let android = tree.element_scroll_group_affine(scroll);
    assert!(android[3] > 1.0, "Android は越境軸を一様 stretch scale（scale_y > 1）: {android:?}");
}

#[test]
fn scroll_quad_getters_expose_affine_and_kind() {
    // present 側が content band quad を組むための getter：scroll Group affine（iOS プロファイル
    // 既定＝素の translate）と ScrollView 判定。
    let (mut tree, scroll, item) = scroll_fixture();
    let _ = tree.render(0.0);

    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    assert_eq!(
        tree.element_scroll_group_affine(scroll),
        [1.0, 0.0, 0.0, 1.0, 0.0, -50.0],
        "既定（Auto→iOS）プロファイルは素の translate"
    );
    assert!(tree.element_is_scroll_view(scroll));
    assert!(!tree.element_is_scroll_view(item));
}
