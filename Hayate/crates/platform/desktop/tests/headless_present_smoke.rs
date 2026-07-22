//! Desktop `Surface` present の headless wgpu スモーク（ADR-0118 / issue #505）。
//!
//! window を開かず、desktop が present に使うのと同じ vello/wgpu 経路で共有 demo fixture
//! （`tasks_tree`）を 1 枚 offscreen に焼き、Tasks の風景が実際にインクとして現れることを
//! 確認する。`try_vello_harness()` 方式で、wgpu アダプタの無い環境（CI 等）では skip する。

// vello/wgpu 経路のテスト — `backend-vello`（default on）を外したビルドでは対象外。
#![cfg(feature = "backend-vello")]

use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_scene_test_support::vello::{render_scene_to_pixels_scaled, try_vello_harness};

/// 共有 fixture を vello で offscreen に 1 枚焼き、暗いインク（Tasks UI のテキスト等）が
/// 現れることを確認する。これが desktop の「静的 1 枚」present 経路のスモーク。
#[test]
fn shared_fixture_paints_tasks_scenery_through_vello() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("[SKIP] wgpu adapter not available; desktop present smoke skipped");
        return;
    };

    let (vw, vh) = TASKS_VIEWPORT;
    let mut tree = tasks_tree("vello");
    tree.render(0.0);
    let graph = tree.committed_frame().snapshot().clone();

    let pixels = render_scene_to_pixels_scaled(&mut harness, &graph, vw as u32, vh as u32, 1.0)
        .expect("offscreen render must succeed once an adapter is present");

    // 「静的 1 枚」が空でないこと: Tasks UI の暗いインク（本文テキスト #262130 等）が
    // まとまった画素数で描かれる。空フレームや clear だけなら 0 に落ちる。
    let dark = pixels
        .chunks_exact(4)
        .filter(|p| p[0] < 128 && p[1] < 128 && p[2] < 128)
        .count();
    assert!(
        dark > 100,
        "expected the Tasks scenery to paint dark ink, got {dark} dark px"
    );
}
