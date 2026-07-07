//! issue #725: per-element layout size イベント（ブラウザ ResizeObserver 相当・ADR-0143）。
//!
//! リスナを登録した要素について、ボーダーボックスサイズが初回確定した commit と
//! 確定サイズが変化した commit で `Event::LayoutResize` を配送する。サイズ非変化の
//! commit・リスナ未登録の要素では発火しない。

use hayate_core::{
    Dimension, DocumentEventKind, ElementId, ElementKind, ElementTree, Event, EventDelivery,
    StyleProp,
};

/// 配送列から LayoutResize を `(target_id, width, height)` に射影する。
fn resize_sizes(deliveries: &[EventDelivery]) -> Vec<(u64, f32, f32)> {
    deliveries
        .iter()
        .filter_map(|d| match &d.event {
            Event::LayoutResize {
                target_id,
                width,
                height,
            } => Some((target_id.to_u64(), *width, *height)),
            _ => None,
        })
        .collect()
}

fn sized_box(tree: &mut ElementTree, id: u64, w: f32, h: f32) -> ElementId {
    let el = tree.element_create(id, ElementKind::View);
    tree.element_set_style(
        el,
        &[
            StyleProp::Width(Dimension::px(w)),
            StyleProp::Height(Dimension::px(h)),
        ],
    );
    el
}

#[test]
fn fires_on_first_layout_for_listening_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
    let box_ = sized_box(&mut tree, 2, 80.0, 40.0);
    tree.element_append_child(root, box_);
    tree.register_listener(box_, DocumentEventKind::LayoutResize);

    tree.render(0.0);

    assert_eq!(resize_sizes(&tree.poll_deliveries()), vec![(2, 80.0, 40.0)]);
}

#[test]
fn does_not_fire_on_a_commit_that_leaves_the_size_unchanged() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
    let box_ = sized_box(&mut tree, 2, 80.0, 40.0);
    tree.element_append_child(root, box_);
    tree.register_listener(box_, DocumentEventKind::LayoutResize);

    tree.render(0.0);
    let _ = tree.poll_deliveries(); // 初回確定ぶんを捨てる

    // 何も変えずに再 commit → サイズ非変化なので発火しない。
    tree.render(16.0);

    assert_eq!(resize_sizes(&tree.poll_deliveries()), Vec::new());
}

#[test]
fn fires_again_when_the_resolved_size_changes() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
    let box_ = sized_box(&mut tree, 2, 80.0, 40.0);
    tree.element_append_child(root, box_);
    tree.register_listener(box_, DocumentEventKind::LayoutResize);

    tree.render(0.0);
    let _ = tree.poll_deliveries();

    // 幅を確定サイズが変わるように更新する。
    tree.element_set_style(box_, &[StyleProp::Width(Dimension::px(120.0))]);
    tree.render(16.0);

    assert_eq!(resize_sizes(&tree.poll_deliveries()), vec![(2, 120.0, 40.0)]);
}

#[test]
fn does_not_fire_for_elements_without_a_listener() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
    let listening = sized_box(&mut tree, 2, 80.0, 40.0);
    let silent = sized_box(&mut tree, 3, 50.0, 30.0);
    tree.element_append_child(root, listening);
    tree.element_append_child(root, silent);
    // `listening` のみ購読。`silent` は購読しない。
    tree.register_listener(listening, DocumentEventKind::LayoutResize);

    tree.render(0.0);

    // 発火は購読要素 1 件のみ。未購読要素の resize は届かない。
    assert_eq!(resize_sizes(&tree.poll_deliveries()), vec![(2, 80.0, 40.0)]);
}
