//! 契約テスト（issue #475, ADR-0080 を native へ延長）。
//!
//! Android/iOS の native ループは surface 生成/リサイズ/回転時に `tree.set_viewport`
//! を Rust 側で直接駆動し、JS（Tsubame / 埋め込み Hermes）を resize 経路から排除する。
//! 本テストはその駆動鎖 — native surface lifecycle イベント → `ViewportMetrics`（物理 px
//! → 論理 viewport, Web 経路と共有）→ `ElementTree::set_viewport` — を、レンダラも JS
//! ホストも介さず core プリミティブだけで再現し、viewport が surface に追従することを
//! host 上で固定する（実機不要）。JS が経路に存在しないことは、この鎖が JS を一切
//! 参照せず成立する事実そのもので担保される。

use hayate_core::{
    ElementTree, SurfaceLifecycleAction, SurfaceLifecycleEvent, SurfaceLifecycleState,
    ViewportMetrics,
};

/// content scale 1.0 で描く現行 Android の物理 px → 論理 viewport 換算（leaf glue と同じ規約）。
fn surface_viewport(width: u32, height: u32) -> (f32, f32) {
    ViewportMetrics::from_physical_size(width as i32, height as i32, 1.0).viewport_size()
}

/// native ループ本体が surface lifecycle action から viewport を駆動する一手を写したもの。
/// `CreateSurface` / `ResizeSurface` のときだけ `tree.set_viewport` を Rust から直接呼ぶ
/// （JS 非経由）。
fn drive(tree: &mut ElementTree, action: SurfaceLifecycleAction, surface: (u32, u32)) {
    match action {
        SurfaceLifecycleAction::CreateSurface => {
            let (vw, vh) = surface_viewport(surface.0, surface.1);
            tree.set_viewport(vw, vh);
        }
        SurfaceLifecycleAction::ResizeSurface { width, height } => {
            let (vw, vh) = surface_viewport(width, height);
            tree.set_viewport(vw, vh);
        }
        _ => {}
    }
}

#[test]
fn surface_creation_drives_viewport_natively() {
    let mut tree = ElementTree::new();
    let mut lifecycle = SurfaceLifecycleState::new();

    let action = lifecycle.handle(SurfaceLifecycleEvent::InitWindow);
    assert_eq!(action, SurfaceLifecycleAction::CreateSurface);
    drive(&mut tree, action, (1080, 1920));

    assert_eq!(tree.viewport(), (1080.0, 1920.0));
}

#[test]
fn surface_rotation_redrives_viewport_natively() {
    let mut tree = ElementTree::new();
    let mut lifecycle = SurfaceLifecycleState::new();

    drive(
        &mut tree,
        lifecycle.handle(SurfaceLifecycleEvent::InitWindow),
        (1080, 1920),
    );
    assert_eq!(tree.viewport(), (1080.0, 1920.0));

    // 回転: 物理 surface の幅と高さが入れ替わる。native が新しい viewport を再駆動する。
    let action = lifecycle.handle(SurfaceLifecycleEvent::WindowResized {
        width: 1920,
        height: 1080,
    });
    assert_eq!(
        action,
        SurfaceLifecycleAction::ResizeSurface {
            width: 1920,
            height: 1080,
        }
    );
    drive(&mut tree, action, (1920, 1080));

    assert_eq!(tree.viewport(), (1920.0, 1080.0));
}

#[test]
fn resize_before_surface_exists_does_not_drive_viewport() {
    let mut tree = ElementTree::new();
    let mut lifecycle = SurfaceLifecycleState::new();
    let default_viewport = tree.viewport();

    // surface 未生成での WindowResized は NoOp。viewport は駆動されない。
    let action = lifecycle.handle(SurfaceLifecycleEvent::WindowResized {
        width: 800,
        height: 600,
    });
    assert_eq!(action, SurfaceLifecycleAction::NoOp);
    drive(&mut tree, action, (800, 600));

    assert_eq!(tree.viewport(), default_viewport);
}
