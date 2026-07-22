//! Deterministic cache-footprint probe for Skia bounded layers.
//!
//! Run with `HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-skia
//! --test perf_probe -- --nocapture` to print the measured source-pixel and cache-byte reduction.

use hayate_core::ElementId;
use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_layer_compositor::GpuBudget;
use hayate_scene_renderer_skia::{new_raster_surface, SkiaLayerPresenter};

#[test]
fn bounded_layer_probe_observes_fewer_raster_pixels_and_cache_bytes() {
    let (viewport_w, viewport_h) = TASKS_VIEWPORT;
    let (width, height) = (viewport_w as u32, viewport_h as u32);
    let mut tree = tasks_tree("skia");
    tree.element_set_transform(ElementId::from_u64(2), Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    let frame = tree.commit_rendered_frame(0.0);
    let mut presenter = SkiaLayerPresenter::new(width, height, 1.0);
    let target = new_raster_surface(width as i32, height as i32).unwrap();
    let _target = presenter
        .present(
            frame.snapshot(),
            frame.layer_topology(),
            &Default::default(),
            [1.0, 1.0, 1.0, 1.0],
            (0.0, 0.0),
            GpuBudget::from_viewports(width, height, 8.0),
            target,
        )
        .unwrap();

    let full_surface_pixels =
        u64::from(width) * u64::from(height) * frame.layer_topology().paint_order().len() as u64;
    let raster_pixels = presenter.last_raster_pixels();
    let cache_bytes = presenter.cached_texture_bytes();
    assert!(
        frame.layer_topology().paint_order().len() > 1,
        "probe needs a non-root layer"
    );
    assert!(
        raster_pixels < full_surface_pixels,
        "bounded caches must raster fewer pixels than one full surface per layer"
    );
    assert_eq!(cache_bytes, raster_pixels * 4);

    if std::env::var_os("HAYATE_PERF_PROBE").is_some() {
        let reduction = (1.0 - raster_pixels as f64 / full_surface_pixels as f64) * 100.0;
        println!(
            "[perf-probe] skia dirty-layer raster px {raster_pixels}/{full_surface_pixels}; \
             cache bytes {cache_bytes}; reduction {reduction:.1}%"
        );
    }
}
