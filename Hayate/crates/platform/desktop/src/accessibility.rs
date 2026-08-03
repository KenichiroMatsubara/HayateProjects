//! Desktop の AccessKit target。
//!
//! lifecycle・差分・Core action 変換は共有 `NativeAccessibilitySession` が所有し、
//! このモジュールは winit window と AccessKit adapter の境界だけを受け持つ。

use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_winit::Adapter;
use hayate_app_host::{
    AppHost, NativeAccessibilityDelivery, NativeAccessibilityHandle,
    NativeAccessibilityMountFailure, NativeAccessibilityTarget,
};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::RenderHostSurface;

const PLATFORM: &str = "desktop";
const ADAPTER_INIT_FAILURE: &str = "adapter-init";

type HandleSlot = Arc<Mutex<Option<NativeAccessibilityHandle>>>;

/// Window event の AccessKit 前処理と outbound update が共有する、一窓一つの adapter。
pub struct DesktopAccessibility {
    adapter: Rc<RefCell<Adapter>>,
}

impl DesktopAccessibility {
    /// Core の window event 処理より先に AccessKit へイベントを渡す。
    pub fn process_window_event(&self, window: &Window, event: &WindowEvent) {
        self.adapter.borrow_mut().process_event(window, event);
    }
}

struct AccessKitTarget {
    adapter: Rc<RefCell<Adapter>>,
}

impl NativeAccessibilityTarget for AccessKitTarget {
    fn update(&mut self, update: TreeUpdate) -> NativeAccessibilityDelivery {
        let mut delivered = false;
        self.adapter.borrow_mut().update_if_active(|| {
            delivered = true;
            update
        });
        if delivered {
            NativeAccessibilityDelivery::Applied
        } else {
            NativeAccessibilityDelivery::Inactive
        }
    }
}

struct Activate {
    handle: HandleSlot,
}

impl ActivationHandler for Activate {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        with_handle(&self.handle, NativeAccessibilityHandle::activate);
        // The shared session publishes the full tree after the next committed frame.
        None
    }
}

struct Action {
    handle: HandleSlot,
}

impl ActionHandler for Action {
    fn do_action(&mut self, request: ActionRequest) {
        with_handle(&self.handle, |handle| handle.action(request));
    }
}

struct Deactivate {
    handle: HandleSlot,
}

impl DeactivationHandler for Deactivate {
    fn deactivate_accessibility(&mut self) {
        with_handle(&self.handle, NativeAccessibilityHandle::deactivate);
    }
}

fn with_handle(slot: &HandleSlot, callback: impl FnOnce(&NativeAccessibilityHandle)) {
    if let Some(handle) = slot
        .lock()
        .expect("desktop accessibility callback slot poisoned")
        .as_ref()
    {
        callback(handle);
    }
}

/// 非表示で作った window に実 adapter を構築し、共有 session へ target を mount する。
/// adapter 構築失敗は renderer を止めず、AppHost の platform/category 観測へ保存する。
pub fn mount_desktop_accessibility(
    event_loop: &ActiveEventLoop,
    window: &Arc<Window>,
    app_host: &mut AppHost<RenderHostSurface>,
    base_dpr: f64,
) -> Option<DesktopAccessibility> {
    let handle_slot = Arc::new(Mutex::new(None));
    let adapter = catch_unwind(AssertUnwindSafe(|| {
        Adapter::with_direct_handlers(
            event_loop,
            window,
            Activate {
                handle: handle_slot.clone(),
            },
            Action {
                handle: handle_slot.clone(),
            },
            Deactivate {
                handle: handle_slot.clone(),
            },
        )
    }))
    .map(|adapter| Rc::new(RefCell::new(adapter)))
    .map_err(|_| NativeAccessibilityMountFailure::new(PLATFORM, ADAPTER_INIT_FAILURE));

    let adapter = match adapter {
        Ok(adapter) => adapter,
        Err(failure) => {
            app_host.try_mount_native_accessibility(Err(failure), base_dpr);
            return None;
        }
    };
    let target: Box<dyn NativeAccessibilityTarget> = Box::new(AccessKitTarget {
        adapter: adapter.clone(),
    });
    let handle = app_host
        .try_mount_native_accessibility(Ok(target), base_dpr)
        .expect("desktop accessibility target was constructed");
    *handle_slot
        .lock()
        .expect("desktop accessibility callback slot poisoned") = Some(handle);

    Some(DesktopAccessibility { adapter })
}
