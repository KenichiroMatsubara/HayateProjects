use hayate_core::{Color, Dimension, DisplayValue, ElementKind, ElementTree, StyleProp};

/// 明示トラック (`grid-template-rows`) を 1 行だけ定義し、そこに収まらない 2 つ目の
/// アイテムが暗黙行へあふれる。暗黙行のサイズは `grid-auto-rows` が決める。これが
/// 適用されなければ暗黙行は auto（=コンテンツ高さ 0）となり、本テストは赤になる。
#[test]
fn grid_auto_rows_sizes_implicit_rows_beyond_explicit_tracks() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let first = tree.element_create(2, ElementKind::View);
    let second = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            // 1 列・明示 1 行。2 つ目のアイテムは暗黙行へ流れる。
            StyleProp::GridTemplateColumns(vec![Dimension::fr(1.0)]),
            StyleProp::GridTemplateRows(vec![Dimension::px(50.0)]),
            StyleProp::GridAutoRows(vec![Dimension::px(30.0)]),
        ],
    );
    for child in [first, second] {
        tree.element_append_child(root, child);
        // 高さは行サイズ（stretch）に委ね、明示しない。
        tree.element_set_style(
            child,
            &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
        );
    }
    tree.render(0.0);

    let first_rect = tree.element_layout_rect(first).expect("first child layout");
    let second_rect = tree.element_layout_rect(second).expect("second child layout");

    // 明示行: y=0, 高さ 50。
    assert!(
        (first_rect.1 - 0.0).abs() < 1.0,
        "first child y={}",
        first_rect.1
    );
    assert!(
        (first_rect.3 - 50.0).abs() < 1.0,
        "first child height={}",
        first_rect.3
    );
    // 暗黙行: grid-auto-rows=30 で y=50, 高さ 30。
    assert!(
        (second_rect.1 - 50.0).abs() < 1.0,
        "second child y={} (expected implicit row at 50)",
        second_rect.1
    );
    assert!(
        (second_rect.3 - 30.0).abs() < 1.0,
        "second child height={} (expected grid-auto-rows 30)",
        second_rect.3
    );
}
