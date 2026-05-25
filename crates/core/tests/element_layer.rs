use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, FlexDirectionValue,
    NodeKind, StyleProp,
};

#[test]
fn element_create_and_append_builds_tree() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let child_a = tree.element_create(ElementKind::View);
    let child_b = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, child_a);
    tree.element_append_child(root, child_b);
    assert_eq!(tree.root(), Some(root));
}

#[test]
fn set_style_routes_layout_and_visual() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    let sg = tree.render();
    // Expect a single Rect node with the layout-computed size.
    let mut found = false;
    for (_, n) in sg.iter() {
        if let NodeKind::Rect { width, height, color, .. } = &n.kind {
            assert!((*width - 100.0).abs() < 0.5);
            assert!((*height - 50.0).abs() < 0.5);
            assert!((color[0] - 1.0).abs() < 1e-3);
            found = true;
        }
    }
    assert!(found, "background rect not emitted");
}

#[test]
fn flex_row_positions_children_with_gap() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let a = tree.element_create(ElementKind::View);
    let b = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(500.0, 200.0);

    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Gap(Dimension::px(10.0)),
            StyleProp::Width(Dimension::px(500.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(Color::new(0.1, 0.1, 0.1, 1.0)),
        ],
    );
    for &child in &[a, b] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
                StyleProp::BackgroundColor(Color::new(0.0, 0.5, 1.0, 1.0)),
            ],
        );
    }

    let sg = tree.render();
    let mut xs: Vec<f32> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { x, width, .. } if (*width - 100.0).abs() < 0.5 => Some(*x),
            _ => None,
        })
        .collect();
    xs.sort_by(|p, q| p.partial_cmp(q).unwrap());
    assert_eq!(xs.len(), 2, "expected two child rects, got {xs:?}");
    assert!((xs[0] - 0.0).abs() < 0.5, "first child x = {}", xs[0]);
    assert!((xs[1] - 110.0).abs() < 0.5, "second child x = {}", xs[1]);
}

#[test]
fn text_element_produces_text_run() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let text = tree.element_create(ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(root, text);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::FontSize(24.0)]);
    tree.element_set_text(text, "Hello");
    assert_eq!(tree.element_get_text(text), "Hello");
    let sg = tree.render();
    let has_text_run = sg
        .iter()
        .any(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }));
    assert!(has_text_run, "no TextRun emitted for text element");
}

#[test]
fn scene_build_walks_absolute_coordinates() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let child = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::PaddingLeft(Dimension::px(20.0)),
            StyleProp::PaddingTop(Dimension::px(20.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, child);
    let sg = tree.render();
    let mut child_pos = None;
    for (_, n) in sg.iter() {
        if let NodeKind::Rect { x, y, width, height, color, .. } = &n.kind {
            if (*width - 50.0).abs() < 0.5
                && (*height - 50.0).abs() < 0.5
                && (color[1] - 1.0).abs() < 1e-3
            {
                child_pos = Some((*x, *y));
            }
        }
    }
    let (x, y) = child_pos.expect("child rect missing");
    assert!((x - 20.0).abs() < 0.5, "child x = {x}");
    assert!((y - 20.0).abs() < 0.5, "child y = {y}");
}

#[test]
fn remove_subtree_drops_children() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let a = tree.element_create(ElementKind::View);
    let b = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, a);
    tree.element_append_child(a, b);
    tree.element_remove(a);
    // After removing `a`, both `a` and `b` should be gone, but root remains.
    assert_eq!(tree.element_kind(root), Some(ElementKind::View));
    assert_eq!(tree.element_kind(a), None);
    assert_eq!(tree.element_kind(b), None);
}
