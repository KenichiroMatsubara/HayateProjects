use std::sync::Arc;

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{
    Blob, Color, DrawCommand, DrawPaint, ElementKind, ElementTree, OverflowValue, PathVerb,
    RenderImage, RenderImageAlphaType, RenderImageFormat, Shadow,
};

#[test]
fn frame_commit_returns_one_renderer_ready_view() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.4, 0.6, 1.0)),
        ],
    );

    let frame = tree.commit_rendered_frame(0.0);

    assert!(!frame.scene().is_empty());
    assert_eq!(frame.layers().first(), Some(&root));
    assert!(frame.content_dirty_layers().contains(&root));
    assert!(frame
        .scroll_inputs()
        .iter()
        .any(|input| input.layer == scroll));
    assert_eq!(frame.has_pending_visual_work(), false);
}

#[test]
fn committed_frame_exposes_finite_logical_raster_bounds_for_each_layer() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 50.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.4, 0.6, 1.0)),
        ],
    );

    let frame = tree.commit_rendered_frame(0.0);
    let bounds = frame
        .layer_raster_bounds()
        .iter()
        .find(|bounds| bounds.layer == root)
        .expect("each committed layer has raster bounds");

    assert_eq!((bounds.origin_x, bounds.origin_y), (0.0, 0.0));
    assert_eq!((bounds.width, bounds.height), (100.0, 50.0));
    assert!(bounds.is_finite());
}

#[test]
fn nested_compositing_layer_is_raster_local_and_excluded_from_parent() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let moving = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, moving);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.4, 0.6, 1.0)),
        ],
    );
    tree.element_set_style(
        moving,
        &[
            StyleProp::Width(Dimension::px(20.0)),
            StyleProp::Height(Dimension::px(10.0)),
            StyleProp::BackgroundColor(Color::new(0.8, 0.2, 0.1, 1.0)),
        ],
    );
    tree.element_set_transform(moving, Some([2.0, 0.0, 0.0, 3.0, 200.0, 300.0]));

    let frame = tree.commit_rendered_frame(0.0);
    let root_bounds = frame
        .layer_raster_bounds()
        .iter()
        .find(|bounds| bounds.layer == root)
        .unwrap();
    let moving_bounds = frame
        .layer_raster_bounds()
        .iter()
        .find(|bounds| bounds.layer == moving)
        .unwrap();

    assert_eq!(
        (
            root_bounds.origin_x,
            root_bounds.origin_y,
            root_bounds.width,
            root_bounds.height
        ),
        (0.0, 0.0, 100.0, 100.0),
        "a child compositing layer must not inflate its parent layer"
    );
    assert_eq!(
        (
            moving_bounds.origin_x,
            moving_bounds.origin_y,
            moving_bounds.width,
            moving_bounds.height,
        ),
        (0.0, 0.0, 20.0, 10.0),
        "the layer's own transform belongs to composite placement, not raster content"
    );
}

#[test]
fn blurred_drop_and_inset_shadows_have_conservative_raster_reach() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BoxShadow(vec![
                Shadow {
                    offset_x: 8.0,
                    offset_y: 8.0,
                    blur: 6.0,
                    spread: 0.0,
                    color: Color::new(0.0, 0.0, 0.0, 0.5),
                    inset: false,
                },
                Shadow {
                    offset_x: 2.0,
                    offset_y: 2.0,
                    blur: 4.0,
                    spread: 1.0,
                    color: Color::new(0.0, 0.0, 0.0, 0.5),
                    inset: true,
                },
            ]),
        ],
    );

    let frame = tree.commit_rendered_frame(0.0);
    let bounds = frame.layer_raster_bounds().first().unwrap();

    assert!(bounds.origin_x <= -0.1 && bounds.origin_y <= -0.1);
    assert!(bounds.origin_x + bounds.width >= 66.1);
    assert!(bounds.origin_y + bounds.height >= 66.1);
    assert!(bounds.is_finite());
}

#[test]
fn draw_path_bounds_are_clipped_by_the_committed_scene_clip() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::Overflow(OverflowValue::Hidden),
        ],
    );
    tree.element_set_draw(
        root,
        vec![DrawCommand::FillPath {
            verbs: vec![PathVerb::Rect {
                x: -50.0,
                y: -40.0,
                width: 200.0,
                height: 180.0,
            }],
            paint: DrawPaint::default(),
        }],
    );

    let frame = tree.commit_rendered_frame(0.0);
    let bounds = frame.layer_raster_bounds().first().unwrap();

    assert_eq!(
        (
            bounds.origin_x,
            bounds.origin_y,
            bounds.width,
            bounds.height
        ),
        (0.0, 0.0, 100.0, 100.0)
    );
}

#[test]
fn draw_path_coordinate_transforms_contribute_to_raster_bounds() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_draw(
        root,
        vec![
            DrawCommand::Translate { dx: 20.0, dy: 30.0 },
            DrawCommand::FillPath {
                verbs: vec![PathVerb::Rect {
                    x: 1.0,
                    y: 2.0,
                    width: 10.0,
                    height: 15.0,
                }],
                paint: DrawPaint::default(),
            },
        ],
    );

    let frame = tree.commit_rendered_frame(0.0);
    let bounds = frame.layer_raster_bounds().first().unwrap();

    assert_eq!(
        (
            bounds.origin_x,
            bounds.origin_y,
            bounds.width,
            bounds.height
        ),
        (21.0, 32.0, 10.0, 15.0)
    );
}

#[test]
fn image_pixels_contribute_their_logical_destination_rect() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::Image);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(30.0)),
        ],
    );
    tree.element_set_image(
        root,
        Arc::new(RenderImage {
            width: 1,
            height: 1,
            format: RenderImageFormat::Rgba8,
            alpha_type: RenderImageAlphaType::Alpha,
            data: Blob::from(vec![255, 255, 255, 255]),
        }),
    );

    let frame = tree.commit_rendered_frame(0.0);
    let bounds = frame.layer_raster_bounds().first().unwrap();

    assert_eq!(
        (
            bounds.origin_x,
            bounds.origin_y,
            bounds.width,
            bounds.height
        ),
        (0.0, 0.0, 40.0, 30.0)
    );
}

#[test]
fn text_glyphs_contribute_finite_conservative_bounds() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
    tree.element_set_text(root, "Raster bounds");
    tree.element_set_style(root, &[StyleProp::FontSize(20.0)]);

    let frame = tree.commit_rendered_frame(0.0);
    let bounds = frame.layer_raster_bounds().first().unwrap();

    assert!(
        bounds.width > 0.0,
        "glyph advances contribute horizontal reach"
    );
    assert!(
        bounds.height >= 20.0,
        "the em box is conservatively covered"
    );
    assert!(bounds.is_finite());
}

#[test]
fn root_and_scroll_layer_bounds_cover_clipped_content_and_fixed_chrome() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(Color::new(0.1, 0.1, 0.1, 1.0)),
        ],
    );
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(500.0)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.6, 0.8, 1.0)),
        ],
    );
    tree.element_set_scroll_offset(scroll, 0.0, 150.0);

    let frame = tree.commit_rendered_frame(0.0);
    let root_bounds = frame
        .layer_raster_bounds()
        .iter()
        .find(|bounds| bounds.layer == root)
        .unwrap();
    let scroll_bounds = frame
        .layer_raster_bounds()
        .iter()
        .find(|bounds| bounds.layer == scroll)
        .unwrap();

    assert_eq!(
        (
            root_bounds.origin_x,
            root_bounds.origin_y,
            root_bounds.width,
            root_bounds.height
        ),
        (0.0, 0.0, 300.0, 200.0),
        "the implicit root layer excludes the nested scroll layer"
    );
    assert_eq!(
        (
            scroll_bounds.origin_x,
            scroll_bounds.origin_y,
            scroll_bounds.width,
            scroll_bounds.height,
        ),
        (0.0, 0.0, 200.0, 100.0),
        "scroll content is viewport-clipped and fixed scrollbar chrome remains covered"
    );
}
