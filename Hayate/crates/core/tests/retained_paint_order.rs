use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use hayate_core::{
    render_scene_graph, Color, Dimension, DisplayValue, DrawOp, ElementKind, ElementTree,
    FlexDirectionValue, PositionValue, PseudoState, RecordingPainter, StyleProp, ViewportCondition,
};

struct ThreadCountingAllocator;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for ThreadCountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK_ALLOCATIONS.with(|tracking| {
            if tracking.get() {
                ALLOCATION_COUNT.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: ThreadCountingAllocator = ThreadCountingAllocator;

fn allocations_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATION_COUNT.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    let result = f();
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
    let allocations = ALLOCATION_COUNT.with(Cell::get);
    (result, allocations)
}

#[test]
fn geometry_and_paint_only_changes_reuse_the_retained_parent_order() {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(1, ElementKind::View);
    let first = tree.element_create(2, ElementKind::View);
    let second = tree.element_create(3, ElementKind::View);
    tree.set_root(parent);
    tree.element_append_child(parent, first);
    tree.element_append_child(parent, second);

    assert_eq!(tree.ordered_children(parent), vec![first, second]);
    let rebuilds = tree.test_paint_order_rebuild_count(parent);

    tree.element_set_style(first, &[StyleProp::Width(Dimension::px(40.0))]);
    assert_eq!(tree.ordered_children(parent), vec![first, second]);
    assert_eq!(tree.test_paint_order_rebuild_count(parent), rebuilds);

    tree.element_set_style(
        second,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    assert_eq!(tree.ordered_children(parent), vec![first, second]);
    assert_eq!(tree.test_paint_order_rebuild_count(parent), rebuilds);
}

#[test]
fn z_index_change_rebuilds_once_on_the_next_consumer() {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(10, ElementKind::View);
    let first = tree.element_create(11, ElementKind::View);
    let second = tree.element_create(12, ElementKind::View);
    tree.set_root(parent);
    tree.element_append_child(parent, first);
    tree.element_append_child(parent, second);

    assert_eq!(tree.ordered_children(parent), vec![first, second]);
    let rebuilds = tree.test_paint_order_rebuild_count(parent);

    tree.element_set_style(first, &[StyleProp::ZIndex(1)]);
    assert_eq!(
        tree.test_paint_order_rebuild_count(parent),
        rebuilds,
        "invalidation must remain lazy until a consumer asks for the order"
    );
    assert_eq!(tree.ordered_children(parent), vec![second, first]);
    assert_eq!(tree.test_paint_order_rebuild_count(parent), rebuilds + 1);

    assert_eq!(tree.ordered_children(parent), vec![second, first]);
    assert_eq!(tree.test_paint_order_rebuild_count(parent), rebuilds + 1);
}

#[test]
fn steady_state_hit_testing_reads_paint_order_without_allocating_or_sorting() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(20, ElementKind::View);
    let back = tree.element_create(21, ElementKind::View);
    let front = tree.element_create(22, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    for id in [root, back, front] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
    }
    tree.element_append_child(root, back);
    tree.element_append_child(root, front);
    tree.element_set_style(front, &[StyleProp::ZIndex(1)]);
    tree.render(0.0);

    assert_eq!(tree.hit_test(50.0, 50.0), Some(front));
    let rebuilds = tree.test_paint_order_rebuild_count(root);
    let (hit, allocations) = allocations_during(|| tree.hit_test(50.0, 50.0));

    assert_eq!(hit, Some(front));
    assert_eq!(allocations, 0, "steady-state hit-test allocated");
    assert_eq!(tree.test_paint_order_rebuild_count(root), rebuilds);
}

#[test]
fn layer_topology_uses_the_same_forward_paint_order_as_scene_lowering() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(30, ElementKind::View);
    let high = tree.element_create(31, ElementKind::ScrollView);
    let low = tree.element_create(32, ElementKind::ScrollView);
    tree.set_root(root);
    tree.set_viewport(100.0, 200.0);
    for id in [root, high, low] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(50.0)),
            ],
        );
    }
    tree.element_append_child(root, high);
    tree.element_append_child(root, low);
    tree.element_set_style(high, &[StyleProp::ZIndex(2)]);
    tree.render(0.0);

    assert_eq!(tree.ordered_children(root), vec![low, high]);
    assert_eq!(tree.frame_layers(), &[root, low, high]);
}

#[test]
fn paint_order_uses_effective_z_index_after_pseudo_state_changes() {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(40, ElementKind::View);
    let first = tree.element_create(41, ElementKind::View);
    let second = tree.element_create(42, ElementKind::View);
    tree.set_root(parent);
    tree.element_append_child(parent, first);
    tree.element_append_child(parent, second);
    tree.element_set_pseudo_style(first, PseudoState::Hover, &[StyleProp::ZIndex(3)]);

    assert_eq!(tree.ordered_children(parent), vec![first, second]);
    let rebuilds = tree.test_paint_order_rebuild_count(parent);

    assert!(tree.hover_enter_element(first));
    assert_eq!(tree.ordered_children(parent), vec![second, first]);
    assert_eq!(tree.test_paint_order_rebuild_count(parent), rebuilds + 1);
}

#[test]
fn changing_an_active_pseudo_style_invalidates_effective_z_index_order() {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(45, ElementKind::View);
    let first = tree.element_create(46, ElementKind::View);
    let second = tree.element_create(47, ElementKind::View);
    tree.set_root(parent);
    tree.element_append_child(parent, first);
    tree.element_append_child(parent, second);
    assert!(tree.hover_enter_element(first));
    assert_eq!(tree.ordered_children(parent), vec![first, second]);

    tree.element_set_pseudo_style(first, PseudoState::Hover, &[StyleProp::ZIndex(3)]);

    assert_eq!(tree.ordered_children(parent), vec![second, first]);
}

#[test]
fn matching_viewport_variant_changes_effective_z_index_order() {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(50, ElementKind::View);
    let first = tree.element_create(51, ElementKind::View);
    let second = tree.element_create(52, ElementKind::View);
    tree.set_root(parent);
    tree.set_viewport(400.0, 200.0);
    tree.element_append_child(parent, first);
    tree.element_append_child(parent, second);

    assert_eq!(tree.ordered_children(parent), vec![first, second]);
    tree.element_set_style_variant(
        first,
        ViewportCondition {
            min_width: Some(300.0),
            ..ViewportCondition::default()
        },
        StyleProp::ZIndex(4),
    );

    assert_eq!(tree.ordered_children(parent), vec![second, first]);
}

#[test]
fn viewport_resize_invalidates_order_when_effective_z_index_flips() {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(60, ElementKind::View);
    let first = tree.element_create(61, ElementKind::View);
    let second = tree.element_create(62, ElementKind::View);
    tree.set_root(parent);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(parent, first);
    tree.element_append_child(parent, second);
    tree.element_set_style_variant(
        first,
        ViewportCondition {
            min_width: Some(300.0),
            ..ViewportCondition::default()
        },
        StyleProp::ZIndex(4),
    );

    assert_eq!(tree.ordered_children(parent), vec![first, second]);
    let rebuilds = tree.test_paint_order_rebuild_count(parent);
    tree.set_viewport(400.0, 200.0);

    assert_eq!(tree.ordered_children(parent), vec![second, first]);
    assert_eq!(tree.test_paint_order_rebuild_count(parent), rebuilds + 1);
}

#[test]
fn equal_z_index_keeps_document_order_and_hit_testing_uses_its_reverse() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(70, ElementKind::View);
    let first = tree.element_create(71, ElementKind::View);
    let second = tree.element_create(72, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    for id in [first, second] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
    }
    tree.element_set_style(second, &[StyleProp::MarginTop(Dimension::px(-100.0))]);
    tree.element_append_child(root, first);
    tree.element_append_child(root, second);
    tree.render(0.0);

    assert_eq!(tree.ordered_children(root), vec![first, second]);
    assert_eq!(tree.hit_test(50.0, 50.0), Some(second));
}

#[test]
fn reparent_invalidates_both_parent_orders_lazily() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(80, ElementKind::View);
    let left = tree.element_create(81, ElementKind::View);
    let right = tree.element_create(82, ElementKind::View);
    let moving = tree.element_create(83, ElementKind::View);
    let resident = tree.element_create(84, ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, left);
    tree.element_append_child(root, right);
    tree.element_append_child(left, moving);
    tree.element_append_child(right, resident);
    assert_eq!(tree.ordered_children(left), vec![moving]);
    assert_eq!(tree.ordered_children(right), vec![resident]);
    let left_rebuilds = tree.test_paint_order_rebuild_count(left);
    let right_rebuilds = tree.test_paint_order_rebuild_count(right);

    tree.element_append_child(right, moving);
    assert_eq!(tree.test_paint_order_rebuild_count(left), left_rebuilds);
    assert_eq!(tree.test_paint_order_rebuild_count(right), right_rebuilds);
    assert!(tree.ordered_children(left).is_empty());
    assert_eq!(tree.ordered_children(right), vec![resident, moving]);
    assert_eq!(tree.test_paint_order_rebuild_count(left), left_rebuilds + 1);
    assert_eq!(
        tree.test_paint_order_rebuild_count(right),
        right_rebuilds + 1
    );
}

#[test]
fn hidden_high_z_node_stays_in_order_but_is_absent_from_paint_and_hit_results() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(90, ElementKind::View);
    let hidden = tree.element_create(91, ElementKind::View);
    let visible = tree.element_create(92, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(root, hidden);
    tree.element_append_child(root, visible);
    tree.element_set_style(
        hidden,
        &[
            StyleProp::Display(DisplayValue::None),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::ZIndex(10),
        ],
    );
    tree.element_set_style(
        visible,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    let hidden_is_absent = tree.render(0.0).iter().all(|(_, node)| match &node.kind {
        hayate_core::NodeKind::Rect { color, .. } => color[0] == 0.0,
        _ => true,
    });

    assert_eq!(tree.ordered_children(root), vec![visible, hidden]);
    assert_eq!(tree.hit_test(50.0, 50.0), Some(visible));
    assert!(hidden_is_absent);
}

#[test]
fn nested_z_index_cannot_escape_its_parent_sibling_order() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(100, ElementKind::View);
    let front_parent = tree.element_create(101, ElementKind::View);
    let back_parent = tree.element_create(102, ElementKind::View);
    let nested_high = tree.element_create(103, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    for id in [front_parent, back_parent, nested_high] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
    }
    tree.element_set_style(front_parent, &[StyleProp::ZIndex(1)]);
    tree.element_set_style(back_parent, &[StyleProp::MarginTop(Dimension::px(-100.0))]);
    tree.element_set_style(nested_high, &[StyleProp::ZIndex(999)]);
    tree.element_append_child(root, front_parent);
    tree.element_append_child(root, back_parent);
    tree.element_append_child(back_parent, nested_high);
    tree.render(0.0);

    assert_eq!(tree.ordered_children(root), vec![back_parent, front_parent]);
    assert_eq!(tree.hit_test(50.0, 50.0), Some(front_parent));
}

#[test]
fn scroll_container_hit_testing_reverses_its_retained_child_order() {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(110, ElementKind::ScrollView);
    let content = tree.element_create(111, ElementKind::View);
    let high = tree.element_create(112, ElementKind::View);
    let low = tree.element_create(113, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    for id in [high, low] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Position(PositionValue::Absolute),
                StyleProp::Top(Dimension::px(60.0)),
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
    }
    tree.element_set_style(high, &[StyleProp::ZIndex(2)]);
    tree.element_append_child(scroll, content);
    tree.element_append_child(content, high);
    tree.element_append_child(content, low);
    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    tree.render(0.0);

    assert_eq!(tree.ordered_children(content), vec![low, high]);
    assert_eq!(tree.hit_test(50.0, 50.0), Some(high));
}

#[test]
fn html_and_accessibility_serialization_keep_document_order() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(120, ElementKind::View);
    let high = tree.element_create(121, ElementKind::Button);
    let low = tree.element_create(122, ElementKind::Button);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    for id in [high, low] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Width(Dimension::px(40.0)),
                StyleProp::Height(Dimension::px(40.0)),
            ],
        );
    }
    tree.element_append_child(root, high);
    tree.element_append_child(root, low);
    tree.element_set_style(high, &[StyleProp::ZIndex(5)]);

    assert_eq!(tree.ordered_children(root), vec![low, high]);
    let resolved_ids: Vec<_> = tree
        .resolved_elements()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(resolved_ids, vec![root, high, low]);

    let update = tree.accessibility_update().expect("accessibility tree");
    let root_node = &update.nodes.last().expect("root node").1;
    let child_ids: Vec<u64> = root_node.children().iter().map(|id| id.0).collect();
    assert_eq!(child_ids, vec![high.to_u64(), low.to_u64()]);
}

#[test]
fn mixed_layout_and_z_index_batch_reorders_the_retained_scene() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(130, ElementKind::View);
    let red = tree.element_create(131, ElementKind::View);
    let blue = tree.element_create(132, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(root, red);
    tree.element_append_child(root, blue);
    tree.element_set_style(
        red,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style(
        blue,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::MarginTop(Dimension::px(-100.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.render(0.0);

    tree.element_set_style(
        red,
        &[StyleProp::Width(Dimension::px(99.0)), StyleProp::ZIndex(2)],
    );
    let scene = tree.render(16.0);
    let mut painter = RecordingPainter::new();
    render_scene_graph(scene, &mut painter);
    let colors: Vec<[f32; 4]> = painter
        .ops()
        .iter()
        .filter_map(|op| match op {
            DrawOp::FillRect { color, .. } => Some(*color),
            _ => None,
        })
        .collect();

    assert_eq!(colors, vec![[0.0, 0.0, 1.0, 1.0], [1.0, 0.0, 0.0, 1.0]]);
}
