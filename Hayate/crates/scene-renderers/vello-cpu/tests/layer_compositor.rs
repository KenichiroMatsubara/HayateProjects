//! CPU（vello_cpu）レイヤ rasterizer / compositor の出力パリティ（issue #699 の追加確認）。
//!
//! vello（wgpu）経路で見つかった premultiplied/straight alpha 取り違えバグ（ADR-0135 追記・
//! `layer_present_parity.rs`）が、tiny-skia crate の実装を写し取った本 crate の
//! `VelloCpuLayerCompositor` にもあるかを確認する。tiny-skia 版は既に確認済み（バグ無し）。

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, Shadow, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer};
use hayate_scene_renderer_vello_cpu::{
    VelloCpuCompositeTarget, VelloCpuLayerCompositor, VelloCpuLayerRasterizer,
    VelloCpuSceneRenderer,
};
use vello_cpu::Pixmap;

const W: u16 = 200;
const H: u16 = 200;
const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn px(v: f32) -> Dimension {
    Dimension::px(v)
}

fn render_full(tree: &ElementTree) -> Pixmap {
    let mut pixmap = Pixmap::new(W, H);
    VelloCpuSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);
    pixmap
}

fn render_layered(tree: &ElementTree, root: ElementId) -> Pixmap {
    let graph = tree.scene_graph();
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let placements = collect_layer_placements(graph, root, &boundaries);

    let mut rasterizer = VelloCpuLayerRasterizer::new(u32::from(W), u32::from(H), 1.0);
    for &layer in tree.frame_layers() {
        let scene = if layer == root {
            extract_root_scene(graph, root, &boundaries)
        } else {
            match extract_layer_scene(graph, layer, &boundaries) {
                Some(s) => s,
                None => continue,
            }
        };
        rasterizer.rasterize(layer, &scene, None).unwrap();
    }

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

    let mut compositor = VelloCpuLayerCompositor::new(1.0);
    let mut target = VelloCpuCompositeTarget {
        pixmap: Pixmap::new(W, H),
        clear: CLEAR,
    };
    compositor.composite(&mut target, &quads).unwrap();
    target.pixmap
}

fn assert_pixmaps_equal(full: &Pixmap, layered: &Pixmap, label: &str) {
    let mut worst = 0u8;
    let mut worst_at = 0usize;
    for (i, (a, b)) in full
        .data_as_u8_slice()
        .iter()
        .zip(layered.data_as_u8_slice().iter())
        .enumerate()
    {
        let d = a.abs_diff(*b);
        if d > worst {
            worst = d;
            worst_at = i;
        }
    }
    // 許容差 4（tiny-skia/vello 版の 2 より緩い）: 半透明 box-shadow の縁で、分解 raster と
    // 全面 raster の AA/丸め順が ±1〜4 でずれる（issue #699 調査で実測——1万6000バイト超が
    // 差分だが最大 4、すべて shadow と不透明 box の境界付近に集中し、系統的な alpha 取り違え
    // （vello/wgpu 版で見つかったバグ）の特徴である「大きく一様な」差ではない）。vello_cpu
    // 自体の raster パイプライン固有の丸め特性とみられる。
    assert!(
        worst <= 4,
        "{label}: 全面 raster と CPU レイヤ合成の出力が一致しない（byte {worst_at} で {worst} 差）"
    );
}

/// tiny-skia 版と同じフィクスチャ（`layer_compositor.rs` の
/// `transform_tree_with_translucent_shadow` 参照）。root 中央配置で、shadow の blur 到達域が
/// 既知の v1 制限（pre-transform 座標がビューポート外）を踏まないようにする。
fn transform_tree_with_translucent_shadow() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.set_viewport(f32::from(W), f32::from(H));
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(f32::from(W))),
            StyleProp::Height(px(f32::from(H))),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(hayate_core::AlignValue::Center),
            StyleProp::JustifyContent(hayate_core::JustifyValue::Center),
            StyleProp::BackgroundColor(Color::new(0.9, 0.85, 0.2, 1.0)),
        ],
    );
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(px(60.0)),
            StyleProp::Height(px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur: 20.0,
                spread: 0.0,
                // 非黒・高彩度必須（黒は straight/premultiplied どちらでも src 項が 0）。
                color: Color::new(1.0, 0.0, 0.0, 0.3),
                inset: false,
            }]),
        ],
    );
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 5.0, 5.0]));
    (tree, boxed)
}

#[test]
fn translucent_box_shadow_layer_cpu_composite_matches_full_raster() {
    let (mut tree, _boxed) = transform_tree_with_translucent_shadow();
    let root = ElementId::from_u64(0);
    let _ = tree.render(0.0);
    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "vello_cpu translucent box-shadow layer (issue #699)",
    );
}
