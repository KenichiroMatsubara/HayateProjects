use hayate_core::{
    Color, Dimension, DisplayValue, ElementKind, ElementTree, StyleProp,
};

#[test]
fn grid_template_columns_fr_splits_space_evenly() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let left = tree.element_create(2, ElementKind::View);
    let right = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::GridTemplateColumns(vec![Dimension::fr(1.0), Dimension::fr(1.0)]),
        ],
    );
    for child in [left, right] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Height(Dimension::px(50.0)),
                StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ],
        );
    }
    tree.render(0.0);

    let left_rect = tree.element_layout_rect(left).expect("left child layout");
    let right_rect = tree.element_layout_rect(right).expect("right child layout");
    assert!((left_rect.0 - 0.0).abs() < 1.0, "left child x={}", left_rect.0);
    assert!((left_rect.2 - 50.0).abs() < 1.0, "left child width={}", left_rect.2);
    assert!((right_rect.0 - 50.0).abs() < 1.0, "right child x={}", right_rect.0);
    assert!((right_rect.2 - 50.0).abs() < 1.0, "right child width={}", right_rect.2);
}

#[test]
fn grid_template_columns_px_uses_fixed_tracks() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let left = tree.element_create(11, ElementKind::View);
    let right = tree.element_create(12, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::GridTemplateColumns(vec![Dimension::px(30.0), Dimension::px(70.0)]),
        ],
    );
    for child in [left, right] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Height(Dimension::px(40.0)),
                StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            ],
        );
    }
    tree.render(0.0);

    let left_rect = tree.element_layout_rect(left).expect("left child layout");
    let right_rect = tree.element_layout_rect(right).expect("right child layout");
    assert!((left_rect.2 - 30.0).abs() < 1.0);
    assert!((right_rect.0 - 30.0).abs() < 1.0);
    assert!((right_rect.2 - 70.0).abs() < 1.0);
}

#[test]
fn grid_item_width_percent_resolves_against_grid_area_not_container() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(20, ElementKind::View);
    let item = tree.element_create(21, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::GridTemplateColumns(vec![
                Dimension::fr(1.0),
                Dimension::fr(1.0),
                Dimension::fr(1.0),
                Dimension::fr(1.0),
            ]),
        ],
    );
    tree.element_append_child(root, item);
    tree.element_set_style(
        item,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.render(0.0);

    let item_rect = tree.element_layout_rect(item).expect("grid item layout");
    assert!(
        (item_rect.2 - 100.0).abs() < 1.0,
        "width:100% on a grid item should resolve to one track (~100px), got {}",
        item_rect.2
    );
}
