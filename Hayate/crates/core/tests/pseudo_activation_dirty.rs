//! Issue #183: pseudo-state activation wired into the dirty pipeline.

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, PseudoState, RecordingPainter,
    StyleProp, StylePropKind, render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    painter.into_ops()
}

#[test]
fn hover_enter_marks_visual_dirty_for_pseudo_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    let (entered, _) = tree.update_pointer_hover(Some(root));
    assert!(entered.contains(&root));
    assert!(
        tree.test_visual_dirty_contains(root),
        "hover enter must mark visual-dirty without waiting for render"
    );
}

#[test]
fn hover_incremental_draw_ops_match_full_rebuild() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(2, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    tree.update_pointer_hover(Some(root));
    tree.render(0.0);
    let incremental_ops = draw_ops(&tree);
    let reference_ops = tree.test_scene_full_rebuild_draw_ops();
    assert_eq!(incremental_ops.len(), reference_ops.len());
    for (got, expected) in incremental_ops.iter().zip(reference_ops.iter()) {
        match (got, expected) {
            (
                DrawOp::FillRect { color: gc, .. },
                DrawOp::FillRect { color: ec, .. },
            ) => {
                for (a, b) in gc.iter().zip(ec.iter()) {
                    assert!((a - b).abs() < 1e-3);
                }
            }
            _ => panic!("draw op mismatch: {got:?} vs {expected:?}"),
        }
    }
}

#[test]
fn idle_frame_after_hover_skips_scene_lowering() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(0.0);
    assert!(tree.test_scene_lowering_walk_count() > 0);

    tree.render(0.0);
    assert_eq!(
        tree.test_scene_lowering_walk_count(),
        0,
        "idle frame with stable interaction must skip re-lowering"
    );
}

#[test]
fn hover_with_font_size_pseudo_marks_shape_dirty() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(10, ElementKind::View);
    let text = tree.element_create(11, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 100.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(80.0)),
        ],
    );
    tree.element_set_text(text, "Hello");
    tree.element_set_style(text, &[StyleProp::FontSize(16.0)]);
    tree.element_set_pseudo_style(text, PseudoState::Hover, &[StyleProp::FontSize(24.0)]);
    tree.render(0.0);

    tree.update_pointer_hover(Some(text));
    assert!(
        tree.test_shape_dirty_contains(text),
        ":hover font-size toggle must mark shape dirty for IFC re-compose"
    );
}

#[test]
fn active_transition_marks_visual_dirty_for_pseudo_element() {
    let mut tree = ElementTree::new();
    let btn = tree.element_create(20, ElementKind::Button);
    tree.set_root(btn);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        btn,
        &[
            StyleProp::Width(Dimension::px(80.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.5, 0.5, 0.5, 1.0)),
        ],
    );
    tree.element_set_pseudo_style(
        btn,
        PseudoState::Active,
        &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 0.5, 1.0))],
    );
    tree.render(0.0);

    tree.on_pointer_down_on(btn, 10.0, 10.0);
    assert!(
        tree.test_visual_dirty_contains(btn),
        ":active start must mark visual-dirty"
    );
}

#[test]
fn focus_pseudo_transition_marks_visual_dirty() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(30, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.element_set_pseudo_style(
        input,
        PseudoState::Focus,
        &[StyleProp::BorderColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );
    tree.render(0.0);

    tree.on_focus(input);
    assert!(
        tree.test_visual_dirty_contains(input),
        ":focus pseudo transition must mark visual-dirty"
    );
}

#[test]
fn caret_blink_phase_flip_marks_visual_dirty() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(40, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.element_focus(input);
    tree.render(1000.0);

    assert!(
        tree.test_tick_cursor_blink(1500.0),
        "blink phase flip must tick"
    );
    assert!(
        tree.test_visual_dirty_contains(input),
        "caret blink phase flip must mark visual-dirty"
    );
}

#[test]
fn unset_pseudo_style_marks_visual_dirty() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(50, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::FontSize(20.0)],
    );
    tree.render(0.0);

    tree.element_unset_pseudo_style(root, PseudoState::Hover, &[StylePropKind::FontSize]);
    assert!(
        tree.test_visual_dirty_contains(root),
        "unset pseudo style must mark visual-dirty"
    );
}

#[test]
fn scroll_offset_marks_visual_dirty() {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(60, ElementKind::ScrollView);
    tree.set_root(scroll);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.render(0.0);

    tree.element_set_scroll_offset(scroll, 0.0, 12.0);
    assert!(
        tree.test_visual_dirty_contains(scroll),
        "scroll offset change must mark visual-dirty"
    );
}
