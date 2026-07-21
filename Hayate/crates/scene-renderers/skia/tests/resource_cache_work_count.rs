//! Paint resource cache の公開 work-count 契約（issue #850）。

use std::sync::Arc;

use hayate_core::element::id::ElementId;
use hayate_core::{
    Blob, Color, Dimension, ElementKind, ElementTree, Node, NodeKind, RenderFont, RenderGlyph,
    RenderImage, RenderImageAlphaType, RenderImageFormat, SceneGraph, StyleProp,
    TextFontAttributes, TextRunData, TextSynthesis,
};
use hayate_layer_compositor::LayerRasterizer;
use hayate_scene_renderer_skia::{new_raster_surface, SkiaLayerRasterizer, SkiaSceneRenderer};

fn image_scene(image: Arc<RenderImage>) -> SceneGraph {
    let mut graph = SceneGraph::new();
    graph.insert(Node {
        kind: NodeKind::Image {
            x: 0.0,
            y: 0.0,
            width: 2.0,
            height: 2.0,
            data: image,
        },
        children: Vec::new(),
    });
    graph
}

fn text_scene_from(resources: &SceneGraph, run: TextRunData) -> SceneGraph {
    let mut graph = resources.empty_projection();
    let text_run = graph.intern_text_run(run);
    graph.insert(Node {
        kind: NodeKind::TextRun {
            x: 0.0,
            y: 20.0,
            color: [0.0, 0.0, 0.0, 1.0],
            text_run,
        },
        children: Vec::new(),
    });
    graph
}

fn text_run(font: RenderFont) -> TextRunData {
    TextRunData {
        font,
        font_size: 20.0,
        font_attributes: TextFontAttributes::default(),
        glyphs: vec![RenderGlyph {
            id: 1,
            x: 0.0,
            y: 0.0,
        }],
        decorations: Vec::new(),
        text: Arc::from("glyph"),
        synthesis: TextSynthesis::default(),
        normalized_coords: Vec::new(),
    }
}

#[test]
fn repeated_raster_reuses_the_render_images_byte_copy_and_sk_image() {
    let image = Arc::new(RenderImage {
        width: 2,
        height: 2,
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        data: Blob::from(vec![255_u8; 2 * 2 * 4]),
    });
    let graph = image_scene(image);
    let mut renderer = SkiaSceneRenderer::new();
    let mut surface = new_raster_surface(2, 2).expect("raster surface");

    renderer.render_scene(&graph, surface.canvas(), [0.0; 4], 1.0);
    renderer.render_scene(&graph, surface.canvas(), [0.0; 4], 1.0);

    let work = renderer.resource_work_counts();
    assert_eq!(work.image_byte_copies, 1);
    assert_eq!(work.sk_images_created, 1);
}

#[test]
fn repeated_raster_reuses_the_text_runs_sk_text_blob() {
    let mut tree = ElementTree::new();
    tree.register_font(
        "Twemoji Smiley Sbix",
        include_bytes!("assets/twemoji_smiley_sbix.ttf").to_vec(),
    );
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    let text = tree.element_create(2, ElementKind::Text);
    tree.element_append_child(root, text);
    tree.element_set_style(
        text,
        &[
            StyleProp::FontFamily("Twemoji Smiley Sbix".to_string()),
            StyleProp::FontSize(40.0),
            StyleProp::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_text(text, "\u{1F601}");
    let graph = tree.render(0.0).clone();
    let mut renderer = SkiaSceneRenderer::new();
    let mut surface = new_raster_surface(100, 100).expect("raster surface");

    renderer.render_scene(&graph, surface.canvas(), [0.0; 4], 1.0);
    renderer.render_scene(&graph, surface.canvas(), [0.0; 4], 1.0);

    assert_eq!(renderer.resource_work_counts().text_blobs_created, 1);
}

#[test]
fn layer_rasterizer_keeps_paint_resources_across_dirty_rasters() {
    let image = Arc::new(RenderImage {
        width: 2,
        height: 2,
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        data: Blob::from(vec![127_u8; 2 * 2 * 4]),
    });
    let graph = image_scene(image);
    let mut rasterizer = SkiaLayerRasterizer::new(2, 2, 1.0);
    let layer = ElementId::from_u64(1);

    rasterizer
        .rasterize(layer, &graph, None)
        .expect("first raster");
    rasterizer
        .rasterize(layer, &graph, None)
        .expect("second raster");

    let work = rasterizer.resource_work_counts();
    assert_eq!(work.image_byte_copies, 1);
    assert_eq!(work.sk_images_created, 1);
}

#[test]
fn image_alpha_metadata_is_part_of_the_cache_identity() {
    let pixels = Blob::from(vec![255_u8; 2 * 2 * 4]);
    let alpha = Arc::new(RenderImage {
        width: 2,
        height: 2,
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        data: pixels.clone(),
    });
    let premultiplied = Arc::new(RenderImage {
        alpha_type: RenderImageAlphaType::Premultiplied,
        data: pixels,
        ..(*alpha).clone()
    });
    let mut renderer = SkiaSceneRenderer::new();
    let mut surface = new_raster_surface(2, 2).expect("raster surface");

    renderer.render_scene(&image_scene(alpha), surface.canvas(), [0.0; 4], 1.0);
    renderer.render_scene(&image_scene(premultiplied), surface.canvas(), [0.0; 4], 1.0);

    assert_eq!(renderer.resource_work_counts().sk_images_created, 2);
}

#[test]
fn font_variation_is_part_of_the_text_blob_cache_identity() {
    let bytes = include_bytes!("assets/twemoji_smiley_sbix.ttf").to_vec();
    let font = RenderFont::new(Blob::from(bytes), 0);
    let base = text_run(font.clone());
    let mut varied = text_run(font);
    varied.normalized_coords = vec![8_192];
    let mut renderer = SkiaSceneRenderer::new();
    let mut surface = new_raster_surface(40, 40).expect("raster surface");
    let resources = SceneGraph::new();

    renderer.render_scene(
        &text_scene_from(&resources, base),
        surface.canvas(),
        [0.0; 4],
        1.0,
    );
    renderer.render_scene(
        &text_scene_from(&resources, varied),
        surface.canvas(),
        [0.0; 4],
        1.0,
    );

    assert_eq!(renderer.resource_work_counts().text_blobs_created, 2);
}

#[test]
fn font_synthesis_is_part_of_the_text_blob_cache_identity() {
    let bytes = include_bytes!("assets/twemoji_smiley_sbix.ttf").to_vec();
    let font = RenderFont::new(Blob::from(bytes), 0);
    let base = text_run(font.clone());
    let mut synthesized = text_run(font);
    synthesized.synthesis = TextSynthesis {
        skew_tangent: Some(0.2),
        embolden: Some(1.0),
    };
    let mut renderer = SkiaSceneRenderer::new();
    let mut surface = new_raster_surface(40, 40).expect("raster surface");
    let resources = SceneGraph::new();

    renderer.render_scene(
        &text_scene_from(&resources, base),
        surface.canvas(),
        [0.0; 4],
        1.0,
    );
    renderer.render_scene(
        &text_scene_from(&resources, synthesized),
        surface.canvas(),
        [0.0; 4],
        1.0,
    );

    assert_eq!(renderer.resource_work_counts().text_blobs_created, 2);
}
