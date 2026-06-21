//! ADR-0081: リサイズ起点のビューポート条件の再解決。

use hayate_core::{
    Color, Dimension, ElementKind, ElementTree, NodeKind, StyleProp, ViewportCondition,
};

#[test]
fn on_resize_crossing_breakpoint_re_resolves_effective_visual_after_commit_frame() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(13201, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(500.0, 800.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );
    tree.commit_frame();

    let before = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        before.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "below breakpoint must use base style before resize"
    );

    tree.on_resize(900.0, 800.0);
    tree.commit_frame();

    let after = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        after.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "after resize crossing breakpoint, commit_frame must settle viewport-conditioned style"
    );
}

#[test]
fn on_resize_crossing_breakpoint_updates_rendered_scene_after_commit_frame() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(13202, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(500.0, 800.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );
    tree.render(0.0);

    let red_channel_before = tree
        .scene_graph()
        .iter()
        .find_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, color, .. } if (*width - 200.0).abs() < 0.5 => Some(color[0]),
            _ => None,
        })
        .expect("root rect in scene");
    assert!(
        (red_channel_before - 1.0).abs() < 1e-3,
        "narrow viewport must render base red"
    );

    tree.on_resize(900.0, 800.0);
    tree.commit_frame();
    tree.render(0.0);

    let red_channel_after = tree
        .scene_graph()
        .iter()
        .find_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, color, .. } if (*width - 200.0).abs() < 0.5 => Some(color[0]),
            _ => None,
        })
        .expect("root rect in scene after resize");
    assert!(
        (red_channel_after - 0.0).abs() < 1e-3,
        "wide viewport must render variant blue after resize + commit_frame"
    );
}

#[test]
fn on_resize_within_same_breakpoint_keeps_effective_visual() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(13204, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(900.0, 800.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );
    tree.commit_frame();

    tree.on_resize(950.0, 850.0);
    tree.commit_frame();

    let visual = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        visual.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "effective visual must remain on the active variant when breakpoint is not crossed"
    );
}

#[test]
fn on_resize_without_viewport_variants_preserves_px_sized_child_layout() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(13203, ElementKind::View);
    let child = tree.element_create(13206, ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, child);
    tree.set_viewport(400.0, 300.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.commit_frame();
    let rect_before = tree.element_layout_rect(child).unwrap();

    tree.on_resize(900.0, 800.0);
    tree.commit_frame();
    let rect_after = tree.element_layout_rect(child).unwrap();

    assert_eq!(
        rect_before, rect_after,
        "px-sized child without viewport variants should keep layout across viewport resize"
    );
}
