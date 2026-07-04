//! Canvas のリサイズ自動検出（ADR-0080）。
//!
//! `ResizeObserver` の配線は `attach_resize_observer`（wasm32 のみ）にある。
//! 寸法計算は純粋関数で、全ターゲットで単体テストされる。

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlCanvasElement, ResizeObserver, ResizeObserverEntry};

/// レイアウトビューポート（CSS px）、バッキングストアサイズ（物理 px）、コンテンツスケール（dpr）。
///
/// Web/Android で共有する `hayate_core::ViewportMetrics` の別名。寸法導出の正本は core 側。
pub use hayate_core::ViewportMetrics as CanvasResizeMetrics;
/// リサイズ通知を出すべきか（サブピクセルの揺れは無視する）。core の正本へ委譲する。
pub use hayate_core::viewport_size_changed;

/// CSS コンテンツボックスと DPR からビューポートとバッファの寸法を導出する。
///
/// 計算自体は `ViewportMetrics::from_logical_size`（Web 経路）に集約されており、本関数は
/// `ResizeObserver` の CSS px 入力をその論理寸法経路へ橋渡しする薄いラッパー。
pub fn canvas_resize_metrics(
    css_width: f32,
    css_height: f32,
    device_pixel_ratio: f64,
) -> CanvasResizeMetrics {
    CanvasResizeMetrics::from_logical_size(css_width, css_height, device_pixel_ratio)
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct ResizeObserverGuard {
    _observer: ResizeObserver,
    _closure: Closure<dyn Fn(js_sys::Array)>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn attach_resize_observer(
    canvas: &HtmlCanvasElement,
    pending_resize: Rc<RefCell<Option<CanvasResizeMetrics>>>,
    last_viewport: Rc<RefCell<(f32, f32)>>,
    request_redraw: Rc<RefCell<Option<js_sys::Function>>>,
) -> Result<ResizeObserverGuard, JsValue> {
    let canvas_for_cb = canvas.clone();
    let closure = Closure::wrap(Box::new(move |entries: js_sys::Array| {
        let entry_value = entries.get(0);
        let Some(entry) = entry_value.dyn_ref::<ResizeObserverEntry>() else {
            return;
        };
        let rect = entry.content_rect();
        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);
        let metrics = canvas_resize_metrics(rect.width() as f32, rect.height() as f32, dpr);
        let next = (metrics.viewport_width, metrics.viewport_height);
        if !viewport_size_changed(*last_viewport.borrow(), next) {
            return;
        }
        *last_viewport.borrow_mut() = next;
        let _ = canvas_for_cb.set_width(metrics.buffer_width);
        let _ = canvas_for_cb.set_height(metrics.buffer_height);
        *pending_resize.borrow_mut() = Some(metrics);
        // `set_width`/`set_height` は HTML5 仕様でバッキングストアを即座にクリアする
        // （透明になる）。on-demand フレームループ（ADR-0080/0126）はこの resize 自体を
        // wake 源として扱わないため、他の入力が来ないままだと `pending_resize` が
        // 消費される次の `render()` が呼ばれず、canvas が空白/古い内容のまま止まる。
        // ポインタ/編集入力と同様にここでも明示的に起こす。
        crate::pointer_input::wake(&request_redraw);
    }) as Box<dyn Fn(js_sys::Array)>);

    let observer = ResizeObserver::new(closure.as_ref().unchecked_ref())?;
    observer.observe(canvas);
    Ok(ResizeObserverGuard {
        _observer: observer,
        _closure: closure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_resize_metrics_uses_css_viewport_and_dpr_scaled_buffer() {
        let metrics = canvas_resize_metrics(400.0, 300.0, 2.0);
        assert_eq!(
            metrics,
            CanvasResizeMetrics {
                viewport_width: 400.0,
                viewport_height: 300.0,
                buffer_width: 800,
                buffer_height: 600,
                content_scale: 2.0,
            }
        );
    }

    #[test]
    fn canvas_resize_metrics_with_unit_dpr_matches_css_size() {
        let metrics = canvas_resize_metrics(640.0, 480.0, 1.0);
        assert_eq!(
            metrics,
            CanvasResizeMetrics {
                viewport_width: 640.0,
                viewport_height: 480.0,
                buffer_width: 640,
                buffer_height: 480,
                content_scale: 1.0,
            }
        );
    }

    #[test]
    fn canvas_resize_metrics_clamps_negative_css_to_zero_viewport() {
        let metrics = canvas_resize_metrics(-10.0, -5.0, 2.0);
        assert_eq!(metrics.viewport_width, 0.0);
        assert_eq!(metrics.viewport_height, 0.0);
        assert_eq!(metrics.buffer_width, 1);
        assert_eq!(metrics.buffer_height, 1);
    }

    #[test]
    fn viewport_size_changed_ignores_sub_pixel_drift() {
        assert!(!viewport_size_changed((800.0, 600.0), (800.2, 600.3)));
    }

    #[test]
    fn viewport_size_changed_detects_css_pixel_changes() {
        assert!(viewport_size_changed((800.0, 600.0), (801.0, 600.0)));
        assert!(viewport_size_changed((800.0, 600.0), (800.0, 601.0)));
    }

    #[test]
    fn adapter_resize_contract_keeps_layout_css_px_and_buffer_physical_px() {
        let metrics = canvas_resize_metrics(800.0, 600.0, 2.0);
        assert_eq!(metrics.viewport_width, 800.0);
        assert_eq!(metrics.viewport_height, 600.0);
        assert_eq!(metrics.content_scale, 2.0);
        assert_eq!(metrics.buffer_width, 1600);
        assert_eq!(metrics.buffer_height, 1200);
    }
}
