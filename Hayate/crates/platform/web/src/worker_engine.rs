//! Production OffscreenCanvas Worker engine.
//!
//! The browser Worker owns this value, so the WASM core, Render Host, selected Scene Renderer,
//! surface resources, and the shared Latest-Wins Frame Pipeline all have the same lifetime. The
//! main thread only transfers structured-clone-safe transport messages.

use hayate_app_host::render_host::SceneRenderer;
use hayate_core::{EditIntent, ElementTree, PointerKind};
use hayate_frame_pipeline::{
    FrameSubmission, LatestWinsFramePipeline, PipelineCommand, PipelineObservation,
};
use hayate_layer_compositor::ResidencyEvent;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use web_sys::OffscreenCanvas;

use crate::backend::{anyhow_to_js, init_worker_render_host, RenderHost};
use crate::ime_bridge::WebImeBridge;
use crate::shared::{element_id_from_f64, kind_from_u32};
use crate::worker_touch_scroll::WorkerTouchScroll;

#[derive(Debug)]
enum WorkerLifecycle {
    Resize {
        width: u32,
        height: u32,
        content_scale: f32,
    },
    Shutdown,
}

/// WASM core + Render Host + selected Scene Renderer owned inside one browser Worker.
#[wasm_bindgen]
pub struct HayateWorkerEngine {
    canvas: OffscreenCanvas,
    backend: RenderHost,
    tree: ElementTree,
    pipeline: LatestWinsFramePipeline<FrameSubmission, WorkerLifecycle>,
    ime: WebImeBridge,
    touch_scroll: WorkerTouchScroll,
    background: [f32; 4],
    detached: bool,
}

#[wasm_bindgen]
impl HayateWorkerEngine {
    pub async fn init(
        canvas: OffscreenCanvas,
        width: u32,
        height: u32,
        dpr: f32,
    ) -> Result<HayateWorkerEngine, JsValue> {
        let content_scale = dpr.max(1.0);
        canvas.set_width(width.max(1));
        canvas.set_height(height.max(1));
        let mut backend = init_worker_render_host(canvas.clone())
            .await
            .map_err(anyhow_to_js)?;
        backend.resize(width.max(1), height.max(1), content_scale);
        let mut tree = ElementTree::new();
        tree.set_viewport(width as f32 / content_scale, height as f32 / content_scale);
        Ok(Self {
            canvas,
            backend,
            tree,
            pipeline: LatestWinsFramePipeline::new(),
            ime: WebImeBridge::default(),
            touch_scroll: WorkerTouchScroll::new(),
            background: [0.0, 0.0, 0.0, 1.0],
            detached: false,
        })
    }

    pub fn element_create(&mut self, id: f64, kind: u32) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.tree.element_create(id as u64, kind_from_u32(kind)?);
        Ok(())
    }

    pub fn set_root(&mut self, id: f64) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.tree.set_root(element_id_from_f64(id));
        Ok(())
    }

    pub fn element_append_child(&mut self, parent: f64, child: f64) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.tree
            .element_append_child(element_id_from_f64(parent), element_id_from_f64(child));
        Ok(())
    }

    pub fn element_insert_before(
        &mut self,
        parent: f64,
        child: f64,
        before: f64,
    ) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.tree.element_insert_before(
            element_id_from_f64(parent),
            element_id_from_f64(child),
            element_id_from_f64(before),
        );
        Ok(())
    }

    pub fn element_remove(&mut self, id: f64) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.tree.element_remove(element_id_from_f64(id));
        Ok(())
    }

    pub fn apply_mutations(
        &mut self,
        ops: &[f64],
        styles: &[f32],
        texts: js_sys::Array,
        draws: &[f32],
    ) -> Result<(), JsValue> {
        self.ensure_attached()?;
        let texts: Vec<String> = texts
            .iter()
            .map(|value| value.as_string().unwrap_or_default())
            .collect();
        hayate_core::wire::apply_mutations(&mut self.tree, ops, styles, &texts, draws)
            .map_err(|error| JsValue::from_str(&error))
    }

    pub fn set_background_color(&mut self, r: f64, g: f64, b: f64) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.background = [r as f32, g as f32, b as f32, 1.0];
        Ok(())
    }

    /// Apply development tuning inside the Worker. Invalid input intentionally keeps compiled
    /// defaults, matching the pre-cutover host behavior.
    pub fn set_tuning(&mut self, json: &str) -> Result<(), JsValue> {
        self.ensure_attached()?;
        let Ok(parsed) = crate::tuning::TuningJson::parse(json) else {
            return Ok(());
        };
        self.tree.set_scroll_tuning(parsed.scroll_tuning());
        self.tree.set_chrome_tuning(parsed.chrome_tuning());
        self.tree.set_scroll_profile(parsed.scroll_profile());
        Ok(())
    }

    /// Commit one immutable frame, admit it to the common Rust policy, and present admitted work.
    pub fn render(&mut self, timestamp_ms: f64) -> Result<Option<js_sys::Promise>, JsValue> {
        self.ensure_attached()?;
        let frame = self.tree.commit_rendered_frame(timestamp_ms);
        self.pipeline
            .admit(PipelineCommand::Frame(
                FrameSubmission::from_committed_frame(&frame),
            ))
            .map_err(|_| JsValue::from_str("frame pipeline is terminal"))?;
        self.start_admitted()
    }

    pub fn resize_surface(
        &mut self,
        width: u32,
        height: u32,
        dpr: f32,
    ) -> Result<Option<js_sys::Promise>, JsValue> {
        self.ensure_attached()?;
        self.pipeline
            .admit(PipelineCommand::Barrier(WorkerLifecycle::Resize {
                width: width.max(1),
                height: height.max(1),
                content_scale: dpr.max(1.0),
            }))
            .map_err(|_| JsValue::from_str("frame pipeline is terminal"))?;
        self.start_admitted()
    }

    pub fn on_pointer_down(&mut self, x: f32, y: f32) -> Result<(), JsValue> {
        self.on_pointer_down_with_kind(x, y, 0)
    }

    pub fn on_pointer_down_with_kind(&mut self, x: f32, y: f32, kind: u32) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.touch_scroll
            .pointer_down(&mut self.tree, x, y, PointerKind::from_u32(kind));
        Ok(())
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) -> Result<(), JsValue> {
        self.on_pointer_move_with_kind(x, y, 0)
    }

    pub fn on_pointer_move_with_kind(&mut self, x: f32, y: f32, kind: u32) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.touch_scroll
            .pointer_move(&mut self.tree, x, y, PointerKind::from_u32(kind));
        Ok(())
    }

    pub fn on_pointer_up(&mut self, x: f32, y: f32) -> Result<(), JsValue> {
        self.on_pointer_up_with_kind(x, y, 0)
    }

    pub fn on_pointer_up_with_kind(&mut self, x: f32, y: f32, kind: u32) -> Result<(), JsValue> {
        self.ensure_attached()?;
        self.touch_scroll
            .pointer_up(&mut self.tree, x, y, PointerKind::from_u32(kind));
        Ok(())
    }

    pub fn on_wheel(&mut self, x: f32, y: f32, delta_x: f32, delta_y: f32) -> Result<(), JsValue> {
        self.ensure_attached()?;
        if let Some(target) = self.tree.hit_test(x, y) {
            self.tree.apply_wheel_delta(target, delta_x, delta_y);
            self.tree.on_wheel(target, delta_x, delta_y);
        }
        Ok(())
    }

    pub fn on_key_down(&mut self, key: &str, modifiers: u32) -> Result<(), JsValue> {
        self.ensure_attached()?;
        if let Some(intent) = crate::edit_keymap::key_to_edit_intent(key, modifiers) {
            if let Some(focused) = self.tree.focused_element() {
                if intent != EditIntent::Paste && self.tree.apply_edit_intent(focused, intent) {
                    return Ok(());
                }
            }
        }
        self.tree.on_key_down(key, modifiers);
        Ok(())
    }

    pub fn dispatch_edit_intent(&mut self, target: f64, intent: &[f64]) -> Result<u32, JsValue> {
        self.ensure_attached()?;
        hayate_core::wire::dispatch_edit_intent(&mut self.tree, target, intent)
            .map(|outcome| match outcome {
                hayate_core::wire::EditDispatchOutcome::Consumed => 0,
                hayate_core::wire::EditDispatchOutcome::Unhandled => 1,
                hayate_core::wire::EditDispatchOutcome::Deferred => 2,
            })
            .map_err(|error| JsValue::from_str(&format!("edit intent protocol: {error:?}")))
    }

    pub fn on_text_input(&mut self, target: f64, text: &str) -> Result<(), JsValue> {
        self.ensure_attached()?;
        let target = if target == 0.0 {
            self.tree.focused_element()
        } else {
            Some(element_id_from_f64(target))
        };
        if let Some(target) = target {
            self.tree.on_text_input(target, text);
        }
        Ok(())
    }

    pub fn ime_wants_keyboard(&self) -> bool {
        self.ime.visible()
    }

    pub fn ime_character_bounds(&self) -> Box<[f32]> {
        let bounds = self.ime.last_bounds();
        vec![bounds.x, bounds.y, bounds.width, bounds.height].into_boxed_slice()
    }

    /// Admit a Shutdown barrier before releasing renderer/surface resources.
    pub fn detach(&mut self) -> Result<Option<js_sys::Promise>, JsValue> {
        if self.detached {
            return Ok(None);
        }
        self.pipeline
            .admit(PipelineCommand::Barrier(WorkerLifecycle::Shutdown))
            .map_err(|_| JsValue::from_str("frame pipeline is terminal"))?;
        self.start_admitted()
    }

    /// Notify the common pipeline that the active renderer/lifecycle command completed.
    pub fn complete_active(&mut self) -> Result<Option<js_sys::Promise>, JsValue> {
        self.pipeline
            .complete_active()
            .map_err(|_| JsValue::from_str("frame pipeline completed without an active command"))?;
        self.start_admitted()
    }

    /// Latch an asynchronous GPU/context failure. A selected Worker renderer is never restarted.
    pub fn fail_active(&mut self, _message: &str) {
        self.backend
            .handle_resource_lifecycle(ResidencyEvent::ContextLost);
        self.pipeline.fail();
    }

    pub fn is_detached(&self) -> bool {
        self.detached
    }

    /// `[accepted, coalesced, dropped, active, pending, failure]`.
    pub fn pipeline_observation(&self) -> Box<[f64]> {
        let PipelineObservation {
            accepted,
            coalesced,
            dropped,
            active,
            pending,
            failure,
        } = self.pipeline.observation();
        vec![
            accepted as f64,
            coalesced as f64,
            dropped as f64,
            f64::from(active),
            pending as f64,
            f64::from(failure),
        ]
        .into_boxed_slice()
    }
}

impl HayateWorkerEngine {
    fn ensure_attached(&self) -> Result<(), JsValue> {
        if self.detached {
            Err(JsValue::from_str("worker engine is detached"))
        } else {
            Ok(())
        }
    }

    fn start_admitted(&mut self) -> Result<Option<js_sys::Promise>, JsValue> {
        let Some(command) = self.pipeline.start_next() else {
            return Ok(None);
        };
        let completion = match command {
            PipelineCommand::Frame(frame) => {
                let scroll_geometry = hayate_layer_compositor::scroll_layer_geometry_from_inputs(
                    &frame.scroll_inputs,
                );
                if let Err(error) = self.backend.present_layers(
                    &frame.scene,
                    &frame.topology,
                    &scroll_geometry,
                    self.background,
                ) {
                    self.pipeline.fail();
                    return Err(anyhow_to_js(error));
                }
                self.backend.submission_completion()
            }
            PipelineCommand::Barrier(WorkerLifecycle::Resize {
                width,
                height,
                content_scale,
            }) => {
                self.canvas.set_width(width);
                self.canvas.set_height(height);
                self.tree
                    .set_viewport(width as f32 / content_scale, height as f32 / content_scale);
                self.backend.resize(width, height, content_scale);
                Box::pin(std::future::ready(Ok(())))
            }
            PipelineCommand::Barrier(WorkerLifecycle::Shutdown) => {
                self.backend
                    .handle_resource_lifecycle(ResidencyEvent::Shutdown);
                self.detached = true;
                Box::pin(std::future::ready(Ok(())))
            }
        };
        self.tree.drive_ime(&mut self.ime);
        Ok(Some(future_to_promise(async move {
            completion.await.map_err(anyhow_to_js)?;
            Ok(JsValue::UNDEFINED)
        })))
    }
}
