//! present 経路の raster work-count 契約（#632・ADR-0086 方式）。
//!
//! 実 `ElementTree` を駆動し、present 側が `frame_layers()` / `frame_layer_dirty()` を
//! `PresentPlanner`（`LayerCache::plan_raster` → `FramePlan`）に通して raster を gating したとき、
//! **clean フレームで raster 呼び出し 0 回・dirty フレームで 1 回**になることをホストで固定する。
//! backend（Android / web）はこの判定をそのまま消費するだけなので、この契約が実機の
//! `render_scene` 呼び出し回数を決める。

use hayate_core::element::style::StyleProp;
use hayate_core::{Color, ElementKind, ElementTree};
use hayate_layer_compositor::PresentPlanner;

/// 1 フレーム回して「raster が走ったか」を返す（backend の present と同じ順序）。単一 root 経路は
/// quad 合成を持たないため、transform 係数だけの変化も保守的に raster トリガへ含める（#633）。
fn pump_frame(tree: &mut ElementTree, planner: &mut PresentPlanner, timestamp_ms: f64) -> bool {
    let _ = tree.render(timestamp_ms);
    let mut trigger = tree.frame_layer_dirty().clone();
    trigger.extend(tree.frame_layer_transform_dirty().iter().copied());
    let plan = planner.plan(tree.frame_layers(), &trigger);
    if plan.needs_raster {
        // backend はここで render_scene（全面 raster）を 1 回呼び、完了を planner に記録する。
        planner.note_full_raster(tree.frame_layers());
        true
    } else {
        false
    }
}

fn demo_tree() -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let child = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, child);
    tree.set_root(root);
    (tree, child)
}

#[test]
fn clean_frame_calls_raster_zero_times() {
    let (mut tree, _child) = demo_tree();
    let mut planner = PresentPlanner::new();

    // 初回はキャッシュ未生成＝raster 1 回（cold）。
    assert!(pump_frame(&mut tree, &mut planner, 0.0), "cold フレームは raster する");

    // 変化のないフレーム：layer_dirty 空・キャッシュ有効 → raster 0 回。
    let mut rasters = 0;
    for i in 1..=5 {
        if pump_frame(&mut tree, &mut planner, i as f64 * 16.0) {
            rasters += 1;
        }
    }
    assert_eq!(rasters, 0, "clean フレームで raster 呼び出しは 0 回");
}

#[test]
fn dirty_frame_calls_raster_exactly_once() {
    let (mut tree, child) = demo_tree();
    let mut planner = PresentPlanner::new();
    let _ = pump_frame(&mut tree, &mut planner, 0.0); // warm

    // 変化フレーム：スタイル変更 → その 1 フレームだけ raster 1 回、以後 0 回。
    tree.element_set_style(child, &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))]);
    assert!(pump_frame(&mut tree, &mut planner, 16.0), "dirty フレームは raster 1 回");
    assert!(!pump_frame(&mut tree, &mut planner, 32.0), "直後の clean フレームは raster 0 回");
}

#[test]
fn surface_invalidation_forces_reraster() {
    // resize / surface 再作成でキャッシュ面が失われたら、clean でも次フレームは raster する。
    let (mut tree, _child) = demo_tree();
    let mut planner = PresentPlanner::new();
    let _ = pump_frame(&mut tree, &mut planner, 0.0); // warm
    assert!(!pump_frame(&mut tree, &mut planner, 16.0));

    planner.invalidate();
    assert!(
        pump_frame(&mut tree, &mut planner, 32.0),
        "invalidate 後の最初のフレームは全面 raster"
    );
    assert!(!pump_frame(&mut tree, &mut planner, 48.0), "その後は再び composite-only");
}
