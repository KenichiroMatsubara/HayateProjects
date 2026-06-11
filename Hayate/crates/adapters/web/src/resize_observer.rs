//! Canvas resize auto-detection (ADR-0080, #133).
//!
//! `ResizeObserver` wiring lives behind `attach_resize_observer` (wasm32 only).
//! Dimension math is pure and unit-tested on all targets.

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

/// Layout viewport (CSS px) and backing-store size (physical px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasResizeMetrics {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub buffer_width: u32,
    pub buffer_height: u32,
}

/// Derive viewport + buffer dimensions from a CSS content box and DPR.
pub fn canvas_resize_metrics(
    css_width: f32,
    css_height: f32,
    device_pixel_ratio: f64,
) -> CanvasResizeMetrics {
    let viewport_width = css_width.max(0.0);
    let viewport_height = css_height.max(0.0);
    let dpr = device_pixel_ratio.max(1.0);
    let buffer_width = (f64::from(viewport_width) * dpr).round().max(1.0) as u32;
    let buffer_height = (f64::from(viewport_height) * dpr).round().max(1.0) as u32;
    CanvasResizeMetrics {
        viewport_width,
        viewport_height,
        buffer_width,
        buffer_height,
    }
}

/// Whether a resize notification should be emitted (sub-pixel tolerant).
pub fn viewport_size_changed(previous: (f32, f32), next: (f32, f32)) -> bool {
    const EPSILON: f32 = 0.5;
    (previous.0 - next.0).abs() > EPSILON || (previous.1 - next.1).abs() > EPSILON
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct ResizeObserverGuard {
    _observer: ResizeObserver,
    _closure: Closure<dyn Fn(js_sys::Array)>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn attach_resize_observer(
    canvas: &HtmlCanvasElement,
    pending_resize: Rc<RefCell<Option<(f32, f32)>>>,
    last_viewport: Rc<RefCell<(f32, f32)>>,
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
        *pending_resize.borrow_mut() = Some(next);
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
}
