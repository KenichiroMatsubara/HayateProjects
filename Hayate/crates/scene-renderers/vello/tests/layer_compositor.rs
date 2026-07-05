//! wgpu quad compositor の warmup / composite 契約（#633・ADR-0130a）。
//!
//! wgpu アダプタが無い環境（CI ホスト）では skip する（`try_vello_harness` 方式）。GPU のある
//! 開発機で、(1) init warmup が全 variant を前倒し生成すること、(2) `composite` が遅延生成経路を
//! 持たない（未 warmup はエラー）こと、(3) レイヤ分解 raster + quad 合成が全面 raster と一致する
//! ことを実行検証する。分解自体の正しさは CPU パリティ（compositor crate）でも常時固定済み。

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementKind, ElementTree};
use hayate_layer_compositor::{
    collect_layer_placements, extract_layer_scene, extract_root_scene, warmup_variants,
    CompositeQuad, LayerCompositor, LayerRasterizer,
};
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, VelloLayerRasterizer, WgpuQuadCompositor,
};
use hayate_scene_test_support::vello::try_vello_harness;

const W: u32 = 200;
const H: u32 = 200;
const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

#[test]
fn warmup_creates_every_pipeline_variant_up_front() {
    let Some(harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let mut compositor = WgpuQuadCompositor::new(harness.device.clone(), harness.queue.clone());
    assert_eq!(compositor.warmed_variant_count(), 0);
    compositor.warmup();
    assert_eq!(
        compositor.warmed_variant_count(),
        warmup_variants().len(),
        "init warmup は surface format × blend の全直積を前倒し生成する（ADR-0130a）"
    );
}

#[test]
fn composite_without_warmup_errors_instead_of_lazily_creating() {
    let Some(harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let mut compositor = WgpuQuadCompositor::new(harness.device.clone(), harness.queue.clone());
    let texture = harness.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let mut target = CompositeTarget {
        view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
        width: W,
        height: H,
        format: wgpu::TextureFormat::Rgba8Unorm,
        clear: CLEAR,
    };
    let result = compositor.composite(&mut target, &[]);
    assert!(
        result.is_err(),
        "未 warmup の variant は遅延生成せずエラーにする（初回スパイクを構造的に防ぐ）"
    );
}

/// レイヤ分解（vello raster + wgpu quad 合成）の出力が全面 raster と一致する（wgpu 実行版）。
#[test]
fn wgpu_layered_composite_matches_full_raster() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };

    // root(灰) > boxed(青 50x50, translate(30,20))。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(W as f32)),
            StyleProp::Height(Dimension::px(H as f32)),
            StyleProp::BackgroundColor(Color::new(0.5, 0.5, 0.5, 1.0)),
        ],
    );
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 30.0, 20.0]));
    let _ = tree.render(0.0);

    // 全面 raster（従来経路）。
    let full = hayate_scene_test_support::vello::render_scene_to_pixels_scaled(
        &mut harness,
        tree.scene_graph(),
        W,
        H,
        1.0,
    )
    .expect("full raster");

    // レイヤ分解 raster + 合成。
    let graph = tree.scene_graph();
    let boundaries: HashSet<_> = tree.frame_layers().iter().copied().collect();
    let mut rasterizer =
        VelloLayerRasterizer::new(harness.device.clone(), harness.queue.clone(), W, H, 1.0).unwrap();
    let root_scene = extract_root_scene(graph, root, &boundaries);
    rasterizer.rasterize(root, &root_scene, None).unwrap();
    let boxed_scene = extract_layer_scene(graph, boxed, &boundaries).unwrap();
    rasterizer.rasterize(boxed, &boxed_scene, None).unwrap();

    let mut compositor = WgpuQuadCompositor::new(harness.device.clone(), harness.queue.clone());
    compositor.warmup();

    let target_texture = harness.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("layered_target"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut target = CompositeTarget {
        view: target_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        width: W,
        height: H,
        format: wgpu::TextureFormat::Rgba8Unorm,
        clear: CLEAR,
    };
    let placements = collect_layer_placements(graph, root, &boundaries);
    let quads: Vec<CompositeQuad<'_, _>> = placements
        .iter()
        .filter_map(|p| {
            rasterizer.texture(p.layer).map(|texture| CompositeQuad {
                layer: p.layer,
                transform: p.transform,
                opacity: 1.0,
                clip: p.clip,
                texture,
            })
        })
        .collect();
    compositor.composite(&mut target, &quads).unwrap();

    let layered = hayate_scene_test_support::vello::readback_rgba8(
        &harness.device,
        &harness.queue,
        &target_texture,
        W,
        H,
    )
    .expect("readback");

    let worst = full
        .iter()
        .zip(layered.iter())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(worst <= 2, "wgpu レイヤ合成が全面 raster と一致しない（最大 {worst} 差）");
}
