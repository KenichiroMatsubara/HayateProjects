//! transform-only フレームの composite-only 契約（#633・work-count 固定）。
//!
//! 実 `ElementTree` を per-layer 経路（`plan_layers` → dirty レイヤだけ raster）で駆動したとき:
//! - transform 係数だけが変わるフレームは **raster 0 回**（quad transform 更新のみ）
//! - レイヤ内容が変わるフレームは **dirty レイヤだけ** raster（`plan_raster` の raster/reuse どおり）
//! - キャッシュ texture サイズは `mark_rasterized_sized`（`note_layer_rasterized`）で計上される

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementKind, ElementTree};
use hayate_layer_compositor::{FramePlan, PresentPlanner};

const LAYER_BYTES: u64 = 200 * 200 * 4;

/// per-layer present を 1 フレーム回し、raster したレイヤ数を返す。
fn pump_frame(tree: &mut ElementTree, planner: &mut PresentPlanner, ts: f64) -> usize {
    let _ = tree.render(ts);
    let plan = planner.plan_layers(tree.frame_layers(), tree.frame_layer_dirty());
    for &layer in &plan.raster {
        planner.note_layer_rasterized(layer, LAYER_BYTES);
    }
    plan.raster.len()
}

fn transform_tree() -> (ElementTree, hayate_core::ElementId, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    let inner = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.element_append_child(boxed, inner);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(20.0)),
            StyleProp::Height(Dimension::px(20.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    (tree, boxed, inner)
}

#[test]
fn transform_only_frames_raster_zero_layers() {
    let (mut tree, boxed, _inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    assert!(
        pump_frame(&mut tree, &mut planner, 0.0) > 0,
        "cold フレームは raster する"
    );

    // transform アニメーション相当：係数だけを毎フレーム変える → vello raster 0 回。
    for frame in 1..=5 {
        let x = frame as f64 * 10.0;
        tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, x, 0.0]));
        let rasters = pump_frame(&mut tree, &mut planner, frame as f64 * 16.0);
        assert_eq!(
            rasters, 0,
            "transform のみのフレーム {frame} で raster が走った"
        );
        // composite-only フレーム（FramePlan でも固定）。
        let plan = planner.plan_layers(tree.frame_layers(), tree.frame_layer_dirty());
        assert!(FramePlan::from_raster(&plan).is_composite_only());
        // quad transform の更新対象としては報告される。
        assert!(
            tree.frame_layer_transform_dirty().contains(&boxed),
            "transform-dirty レイヤは quad 更新対象として捕捉される"
        );
    }
}

#[test]
fn content_change_rerasters_only_the_dirty_layer() {
    let (mut tree, boxed, inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    let _ = pump_frame(&mut tree, &mut planner, 0.0);

    // レイヤ内容（boxed 内の inner）の変化 → boxed レイヤだけ再 raster、root は reuse。
    tree.element_set_style(
        inner,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    let _ = tree.render(16.0);
    let plan = planner.plan_layers(tree.frame_layers(), tree.frame_layer_dirty());
    assert_eq!(plan.raster, vec![boxed], "dirty レイヤ（boxed）だけ raster");
    assert!(
        plan.reuse.contains(&tree.frame_layers()[0]),
        "root レイヤは reuse"
    );
}

#[test]
fn layer_texture_bytes_are_accounted() {
    // AC: キャッシュ texture のサイズは mark_rasterized_sized で計上される（GPU 予算 slice の前提）。
    let (mut tree, _boxed, _inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    let rastered = pump_frame(&mut tree, &mut planner, 0.0);
    assert_eq!(
        planner.cached_bytes(),
        rastered as u64 * LAYER_BYTES,
        "raster したレイヤ数 × texture バイトが台帳に計上される"
    );
}
