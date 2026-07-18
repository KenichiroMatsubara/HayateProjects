//! per-layer present（#690）と全面 raster の golden ピクセル一致（#691・ADR-0125/0127）。
//!
//! `wgpu_layered_composite_matches_full_raster`（`layer_compositor.rs`）は静的な単一 transform
//! レイヤで分解パリティを固定した。ここでは #690 が実際にレイヤ昇格/降格・GPU raster・quad 合成を
//! 通す構成——(1) 2 要素が同一フレームで同時に transition する（#680 実機回帰、`AddForm.tsx` の
//! `seg()` と同型）、(2) scroll コンテナ——で「全面 raster（layer-present OFF 相当）」と「レイヤ分解
//! raster + wgpu quad 合成（layer-present ON 相当）」が画素単位で一致することを固定する。wgpu
//! アダプタが無い環境（CI ホスト）では skip する。

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, Shadow, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::{
    collect_layer_placements, extract_layer_scene, extract_root_scene, CompositeQuad,
    LayerCompositor, LayerRasterizer,
};
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, VelloLayerRasterizer, WgpuQuadCompositor,
};
use hayate_scene_test_support::vello::{
    readback_rgba8, render_scene_to_pixels_scaled, try_vello_harness, VelloHarness,
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
fn assert_layered_matches_full(
    harness: &mut VelloHarness,
    tree: &ElementTree,
    root: ElementId,
    label: &str,
) {
    let graph = tree.scene_graph();
    let full = render_scene_to_pixels_scaled(harness, graph, W, H, 1.0).expect("full raster");

    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let mut rasterizer =
        VelloLayerRasterizer::new(harness.device.clone(), harness.queue.clone(), W, H, 1.0)
            .unwrap();
    for &layer in tree.frame_layers() {
        let extracted = if layer == root {
            Some(extract_root_scene(graph, root, &boundaries))
        } else {
            extract_layer_scene(graph, layer, &boundaries)
        };
        if let Some(extracted) = extracted {
            if layer == root {
                rasterizer.rasterize(layer, &extracted, None).unwrap();
            } else {
                let bounds = tree
                    .committed_frame()
                    .layer_raster_bounds()
                    .iter()
                    .find(|bounds| bounds.layer == layer)
                    .copied()
                    .expect("each promoted layer has Core raster bounds");
                rasterizer
                    .rasterize_in_bounds(layer, &extracted, bounds, None)
                    .unwrap();
            }
        }
    }

    let mut compositor = WgpuQuadCompositor::new(harness.device.clone(), harness.queue.clone());
    compositor.warmup();

    let target_texture = harness.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("layer_present_parity_target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
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

    let layered =
        readback_rgba8(&harness.device, &harness.queue, &target_texture, W, H).expect("readback");

    let worst = full
        .iter()
        .zip(layered.iter())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
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

    assert_layered_matches_full(
        &mut harness,
        &tree,
        root,
        "dual transition mid-frame (t=16ms of 160ms)",
    );

    // on-demand ループが idle に落ちるまで駆動する（転送完了・レイヤ降格まで）。
    let mut t = 32.0;
    let mut frames = 0;
    while tree.has_pending_visual_work() {
        let _ = tree.render(t);
        t += 16.0;
        frames += 1;
        assert!(frames < 200, "有限フレームで idle に落ちなければならない");
    }

    assert_layered_matches_full(
        &mut harness,
        &tree,
        root,
        "dual transition settled (post-transition)",
    );
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
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(150.0)), StyleProp::Height(px(100.0))],
    );
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

/// #699 回帰用フィクスチャ: transition でレイヤ昇格する要素に半透明（alpha 0.3）の
/// ぼかし box-shadow を持たせる。`dual_transition_tree`/`scrolled_scroll_view_tree` は
/// 不透明色（alpha=1.0）しか使わないため、レイヤ texture が straight alpha なのに
/// premultiplied 前提で合成される #699 のバグを検出できなかった（alpha=1 では
/// premultiplied も straight も同じ値になり差が出ない）。シャドウ色は非黒（彩度あり）で
/// なければならない——黒（0,0,0）は straight/premultiplied どちらで解釈しても src 項が
/// 0 のままで差が出ず、このクラスのバグを検出できない（実際に一度黒で書いて検出漏れを確認済み）。
fn single_transitioning_box_with_translucent_shadow_tree() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let a = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, a);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(hayate_core::AlignValue::Center),
            StyleProp::JustifyContent(hayate_core::JustifyValue::Center),
            StyleProp::BackgroundColor(Color::new(0.9, 0.85, 0.2, 1.0)),
        ],
    );
    tree.element_set_style(
        a,
        &[
            StyleProp::Width(px(60.0)),
            StyleProp::Height(px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::TransitionDuration(160.0),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur: 20.0,
                spread: 0.0,
                // 非黒・高彩度: straight を premultiplied として合成すると src 項を alpha で
                // 減衰し忘れる（#699）。黒（0,0,0）は src 項が恒等的に 0 で差が出ない。
                color: Color::new(1.0, 0.0, 0.0, 0.3),
                inset: false,
            }]),
        ],
    );
    let _ = tree.render(0.0);
    (tree, root, a)
}

#[test]
fn layered_present_matches_full_raster_for_translucent_box_shadow_during_transition() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let (mut tree, root, a) = single_transitioning_box_with_translucent_shadow_tree();

    // 補間対象プロパティ（背景色）を変えて active transition を発生させ、a をレイヤへ昇格させる
    // （ADR-0125 の compositing trigger）。box-shadow 自体の値は変えない——検査したいのは
    // 「半透明シャドウを持つ要素がレイヤ化されたときの合成」であって transition の値ではない。
    // `frame_layers()` への反映は 1 フレーム遅れる（`capture_frame_layers` は scene_build 直前の
    // dirty スナップショットから捕捉するため）——render(16.0) 直後はまだ root だけで、a が
    // 実際にレイヤとして分解 raster されるのは次の render 呼び出し以降。
    tree.element_set_style(
        a,
        &[StyleProp::BackgroundColor(Color::new(0.2, 0.2, 0.2, 1.0))],
    );
    let _ = tree.render(16.0); // transition 開始（この時点ではまだ a はレイヤ化されていない）
    let _ = tree.render(32.0); // a がレイヤへ昇格し、分解 raster + quad 合成の経路を通る

    assert_layered_matches_full(
        &mut harness,
        &tree,
        root,
        "translucent box-shadow while element is layer-promoted (issue #699)",
    );
}

#[test]
fn layered_present_matches_full_raster_for_nested_layers_with_local_origins() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let outer = tree.element_create(1, ElementKind::View);
    let inner = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, outer);
    tree.element_append_child(outer, inner);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.2, 0.2, 1.0)),
        ],
    );
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(px(90.0)),
            StyleProp::Height(px(80.0)),
            StyleProp::BackgroundColor(Color::new(0.1, 0.6, 0.3, 1.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(px(35.0)),
            StyleProp::Height(px(30.0)),
            StyleProp::BackgroundColor(Color::new(0.8, 0.2, 0.6, 1.0)),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: -5.0,
                offset_y: 4.0,
                blur: 4.0,
                spread: 0.0,
                color: Color::new(0.2, 0.4, 1.0, 0.4),
                inset: false,
            }]),
        ],
    );
    tree.element_set_transform(outer, Some([1.0, 0.0, 0.0, 1.0, 35.0, 25.0]));
    tree.element_set_transform(inner, Some([1.0, 0.0, 0.0, 1.0, 18.0, 12.0]));
    let _ = tree.render(0.0);
    assert!(tree.frame_layers().contains(&outer));
    assert!(tree.frame_layers().contains(&inner));

    assert_layered_matches_full(
        &mut harness,
        &tree,
        root,
        "nested layers with shadow-expanded local origin",
    );
}
