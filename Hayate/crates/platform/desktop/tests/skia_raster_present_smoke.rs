//! Desktop skia raster present の headless スモーク（issue #801・ADR-0146 §3）。
//!
//! window を開かず、desktop の skia CPU present（softbuffer）が使うのと同じ
//! `raster_frame_xrgb` 経路で共有 demo fixture（`tasks_tree`）を 1 枚焼き、Tasks の風景が
//! softbuffer の 0RGB ピクセルとして実際に現れることを確認する。GPU/wgpu アダプタが
//! 一切無い環境（CI 等）でも skip せず常に走る — これが「GPU が使えない環境でも desktop
//! が起動する」経路の証明。

use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_platform_desktop::skia_present::raster_frame_xrgb;

/// 共有 fixture を skia raster で offscreen に 1 枚焼き、暗いインク（Tasks UI のテキスト等）
/// が現れることを確認する。vello 側の `headless_present_smoke` と対になる skia 経路の
/// スモーク。
#[test]
fn shared_fixture_paints_tasks_scenery_through_skia_raster() {
    let (vw, vh) = TASKS_VIEWPORT;
    let mut tree = tasks_tree("skia");
    let graph = tree.render(0.0).clone();

    let clear = hayate_platform_desktop::CLEAR_COLOR;
    let pixels = raster_frame_xrgb(&graph, vw as u32, vh as u32, 1.0, clear);
    assert_eq!(pixels.len(), (vw as usize) * (vh as usize));

    // softbuffer の 0RGB（上位バイト未使用、R<<16 | G<<8 | B）で暗いインクを数える。
    let dark = pixels
        .iter()
        .filter(|&&p| {
            let r = (p >> 16) & 0xff;
            let g = (p >> 8) & 0xff;
            let b = p & 0xff;
            r < 128 && g < 128 && b < 128
        })
        .count();
    assert!(
        dark > 100,
        "expected the Tasks scenery to paint dark ink, got {dark} dark px"
    );

    // 余白は clear color（#f1ede3 相当）で塗られている — 左上隅は content の外。
    let corner = pixels[0];
    let r = (corner >> 16) & 0xff;
    assert!(r > 200, "corner must be the light clear color, got {corner:#010x}");
}
