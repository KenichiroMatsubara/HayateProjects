//! compositing layer ツリー＋`layer_dirty` のコア統合（ADR-0125 コア半分・#609）。
//!
//! 公開インターフェース（`ElementTree`）越しに、compositing trigger（transform group / scroll
//! コンテナ）からレイヤ境界が自動判定され、レイヤ id が境界要素の `ElementId` に一致し、要素 dirty が
//! 内包する最近接レイヤへ `layer_dirty` として流れることを固定する。純粋な境界判定/導出ロジック自体は
//! `element::compositing` の単体テストにある（ElementTree 非依存）。

use std::collections::HashSet;

use hayate_core::{Color, ElementKind, ElementTree};
use hayate_core::element::style::StyleProp;

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
