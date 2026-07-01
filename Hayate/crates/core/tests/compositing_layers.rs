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
