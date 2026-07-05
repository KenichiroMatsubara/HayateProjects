//! per-layer present（#690）と全面 raster の golden ピクセル一致（#691・ADR-0125/0127）。
//!
//! `wgpu_layered_composite_matches_full_raster`（`layer_compositor.rs`）は静的な単一 transform
//! レイヤで分解パリティを固定した。ここでは #690 が実際にレイヤ昇格/降格・GPU raster・quad 合成を
//! 通す構成——(1) 2 要素が同一フレームで同時に transition する（#680 実機回帰、`AddForm.tsx` の
//! `seg()` と同型）、(2) scroll コンテナ——で「全面 raster（layer-present OFF 相当）」と「レイヤ分解
//! raster + wgpu quad 合成（layer-present ON 相当）」が画素単位で一致することを固定する。wgpu
//! アダプタが無い環境（CI ホスト）では skip する。

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::{
    collect_layer_placements, extract_layer_scene, extract_root_scene, CompositeQuad,
    LayerCompositor, LayerRasterizer,
};
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, VelloLayerRasterizer, WgpuQuadCompositor,
};
use hayate_scene_test_support::vello::{
    render_scene_to_pixels_scaled, try_vello_harness, readback_rgba8, VelloHarness,
};

const W: u32 = 200;
const H: u32 = 200;
const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn px(v: f32) -> Dimension {
    Dimension::px(v)
}

/// レイヤ分解 raster + wgpu quad 合成が全面 raster と画素単位で一致することを検査する。
/// `tree.frame_layers()`（root 暗黙レイヤ込み）をそのままレイヤ集合として使う——#690 の
/// `present_layers` が実運用で通す経路と同じ分解。
fn assert_layered_matches_full(harness: &mut VelloHarness, tree: &ElementTree, root: ElementId, label: &str) {
    let graph = tree.scene_graph();
    let full = render_scene_to_pixels_scaled(harness, graph, W, H, 1.0).expect("full raster");

    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let mut rasterizer =
        VelloLayerRasterizer::new(harness.device.clone(), harness.queue.clone(), W, H, 1.0).unwrap();
    for &layer in tree.frame_layers() {
        let extracted = if layer == root {
            Some(extract_root_scene(graph, root, &boundaries))
        } else {
            extract_layer_scene(graph, layer, &boundaries)
        };
        if let Some(extracted) = extracted {
            rasterizer.rasterize(layer, &extracted).unwrap();
        }
    }

    let mut compositor = WgpuQuadCompositor::new(harness.device.clone(), harness.queue.clone());
    compositor.warmup();

    let target_texture = harness.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("layer_present_parity_target"),
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

    let layered = readback_rgba8(&harness.device, &harness.queue, &target_texture, W, H).expect("readback");

    let worst = full.iter().zip(layered.iter()).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    assert!(
        worst <= 2,
        "{label}: レイヤ分解 raster + wgpu quad 合成が全面 raster と一致しない（最大 {worst} 差）"
    );
}

/// #680 実機回帰の再現構成: 2 要素（優先度セグメントボタン相当）が同一フレームで同時に transition
/// を開始し、同じ duration（160ms・`Tsubame/examples/todo/src/ui/styles.ts` の `EASE` と同値）で
/// 同時に終わる。root > [a, b] の横並びで、a は緑→灰、b は灰→緑へ同時に切り替わる。
fn dual_transition_tree() -> (ElementTree, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let a = tree.element_create(1, ElementKind::View);
    let b = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, a);
    tree.element_append_child(root, b);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(hayate_core::FlexDirectionValue::Row),
            StyleProp::AlignItems(hayate_core::AlignValue::Center),
            StyleProp::JustifyContent(hayate_core::JustifyValue::Center),
            StyleProp::Gap(px(10.0)),
            StyleProp::BackgroundColor(Color::new(0.9, 0.9, 0.9, 1.0)),
        ],
    );
    let active = Color::new(0.0, 1.0, 0.0, 1.0);
    let inactive = Color::new(0.5, 0.5, 0.5, 1.0);
    let common = |bg: Color| {
        vec![
            StyleProp::Width(px(40.0)),
            StyleProp::Height(px(40.0)),
            StyleProp::BackgroundColor(bg),
            StyleProp::TransitionDuration(160.0),
        ]
    };
    tree.element_set_style(a, &common(active));
    tree.element_set_style(b, &common(inactive));
    (tree, root, a, b)
}

#[test]
fn layered_present_matches_full_raster_during_and_after_dual_transition() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };

    let (mut tree, root, a, b) = dual_transition_tree();
    let _ = tree.render(0.0);

    // クリック相当: a を非アクティブへ、b をアクティブへ同一フレームで同時に切り替える
    // （`AddForm.tsx` の `onClick={() => props.onPrio(prio)}` 再描画と同型・#680）。
    let active = Color::new(0.0, 1.0, 0.0, 1.0);
    let inactive = Color::new(0.5, 0.5, 0.5, 1.0);
    tree.element_set_style(a, &[StyleProp::BackgroundColor(inactive)]);
    tree.element_set_style(b, &[StyleProp::BackgroundColor(active)]);
    let _ = tree.render(16.0); // transition 開始

    assert_layered_matches_full(&mut harness, &tree, root, "dual transition mid-frame (t=16ms of 160ms)");

    // on-demand ループが idle に落ちるまで駆動する（転送完了・レイヤ降格まで）。
    let mut t = 32.0;
    let mut frames = 0;
    while tree.has_pending_visual_work() {
        let _ = tree.render(t);
        t += 16.0;
        frames += 1;
        assert!(frames < 200, "有限フレームで idle に落ちなければならない");
    }

    assert_layered_matches_full(&mut harness, &tree, root, "dual transition settled (post-transition)");
}

/// scroll(150x100) 直下に可視域を超える内容(400px)を持つツリーをスクロール済み状態で render する。
fn scrolled_scroll_view_tree() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::BackgroundColor(Color::new(0.9, 0.85, 0.2, 1.0)),
        ],
    );
    tree.element_set_style(scroll, &[StyleProp::Width(px(150.0)), StyleProp::Height(px(100.0))]);
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(px(150.0)),
            StyleProp::Height(px(400.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.3, 0.8, 1.0)),
        ],
    );
    let _ = tree.render(0.0);
    tree.element_set_scroll_offset(scroll, 0.0, 120.0);
    let _ = tree.render(16.0);
    (tree, root)
}

#[test]
fn layered_present_matches_full_raster_for_a_scrolled_container() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let (tree, root) = scrolled_scroll_view_tree();
    assert_layered_matches_full(&mut harness, &tree, root, "scrolled scroll-view");
}
