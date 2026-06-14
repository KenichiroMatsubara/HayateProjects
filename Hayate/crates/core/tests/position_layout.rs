use hayate_core::{
    Color, Dimension, DisplayValue, ElementKind, ElementTree, FlexDirectionValue, PositionValue,
    StyleProp,
};

// `position: absolute` takes the element out of normal flow and positions it by
// its insets relative to the positioned ancestor (ADR-0091, issue #205). The
// in-flow sibling reflows as if the absolute element were absent.
#[test]
fn absolute_element_leaves_flow_and_positions_at_inset() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let abs = tree.element_create(2, ElementKind::View);
    let flow = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );

    tree.element_append_child(root, abs);
    tree.element_set_style(
        abs,
        &[
            StyleProp::Position(PositionValue::Absolute),
            StyleProp::Top(Dimension::px(10.0)),
            StyleProp::Left(Dimension::px(20.0)),
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );

    tree.element_append_child(root, flow);
    tree.element_set_style(
        flow,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.render(0.0);

    let abs_rect = tree.element_layout_rect(abs).expect("absolute layout");
    assert!((abs_rect.0 - 20.0).abs() < 1.0, "absolute x={}", abs_rect.0);
    assert!((abs_rect.1 - 10.0).abs() < 1.0, "absolute y={}", abs_rect.1);
    assert!((abs_rect.2 - 30.0).abs() < 1.0, "absolute w={}", abs_rect.2);
    assert!((abs_rect.3 - 40.0).abs() < 1.0, "absolute h={}", abs_rect.3);

    // The in-flow sibling sits at the top of the column: the absolute element
    // created no space, so `flow` is not pushed down by `abs`'s 40px height.
    let flow_rect = tree.element_layout_rect(flow).expect("in-flow layout");
    assert!(
        flow_rect.1.abs() < 1.0,
        "in-flow sibling must start at y=0 (absolute element out of flow), got y={}",
        flow_rect.1
    );
}
