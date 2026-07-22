//! demo-fixtures の共有シーン（"Tasks" 画面）を Skia で描く per-renderer golden
//! （ADR-0146 §8: レンダラ間ピクセル比較はしない・自分の過去出力との回帰のみ固定）。
//!
//! golden 更新: `HAYATE_UPDATE_GOLDEN=1 cargo test -p hayate-scene-renderer-skia --test demo_fixture_golden`

mod support;

use std::path::PathBuf;

use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_scene_test_support::golden::assert_pixels_match_golden;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"))
}

#[test]
fn tasks_screen_matches_golden() {
    let (vw, vh) = TASKS_VIEWPORT;
    let mut tree = tasks_tree("skia");
    tree.render(0.0);
    let graph = tree.committed_frame().snapshot().clone();
    let pixels = support::render_scene_to_pixels_scaled(&graph, vw as u32, vh as u32, 1.0);
    assert_pixels_match_golden(&golden_path("tasks_screen"), &pixels, vw as u32, vh as u32);
}
