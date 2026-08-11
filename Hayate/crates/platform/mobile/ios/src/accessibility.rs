//! iOS `UIView` target for the shared native accessibility session.
//!
//! Lifecycle, incremental baseline, root DPR transform, and Core action mapping stay in
//! `NativeAccessibilitySession`. This leaf only mounts AccessKit on one UIKit view and forwards
//! AccessKit callbacks to the session mailbox.

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_ios::SubclassingAdapter;
use hayate_app_host::{
    NativeAccessibilityDelivery, NativeAccessibilityHandle, NativeAccessibilityMountFailure,
    NativeAccessibilitySession, NativeAccessibilityTarget,
};

const PLATFORM: &str = "ios";
const ADAPTER_MOUNT_FAILURE: &str = "adapter-mount";

#[derive(Default)]
struct CallbackSlot {
    handle: Option<NativeAccessibilityHandle>,
    pending_activation: bool,
}

type HandleSlot = Arc<Mutex<CallbackSlot>>;

struct IosTarget {
    adapter: SubclassingAdapter,
}

impl NativeAccessibilityTarget for IosTarget {
    fn update(&mut self, update: TreeUpdate) -> NativeAccessibilityDelivery {
        match self.adapter.update_if_active(|| update) {
            Some(events) => {
                events.raise();
                NativeAccessibilityDelivery::Applied
            }
            None => NativeAccessibilityDelivery::Inactive,
        }
    }
}

struct Activate {
    handle: HandleSlot,
}

impl ActivationHandler for Activate {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        let handle = {
            let mut slot = self
                .handle
                .lock()
                .expect("iOS accessibility callback slot poisoned");
            match slot.handle.clone() {
                Some(handle) => Some(handle),
                None => {
                    slot.pending_activation = true;
                    None
                }
            }
        };
        if let Some(handle) = handle {
            handle.activate();
        }
        // The shared session publishes a full tree after the next committed frame.
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
    let handle = slot
        .lock()
        .expect("iOS accessibility callback slot poisoned")
        .handle
        .clone();
    if let Some(handle) = handle.as_ref() {
        callback(handle);
    }
}

/// Mount one real AccessKit target on one `UIView`-backed native surface.
///
/// Construction failures disable accessibility for this surface only. The caller logs the typed
/// failure and continues rendering.
///
/// # Safety
///
/// `view` must be a valid, unreleased `UIView` pointer for the lifetime of the returned session.
pub(crate) unsafe fn mount_ios_accessibility(
    view: *mut c_void,
    base_dpr: f64,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Result<NativeAccessibilitySession, NativeAccessibilityMountFailure> {
    if view.is_null() {
        return Err(NativeAccessibilityMountFailure::new(
            PLATFORM,
            ADAPTER_MOUNT_FAILURE,
        ));
    }

    let handle_slot = Arc::new(Mutex::new(CallbackSlot::default()));
    let adapter = catch_unwind(AssertUnwindSafe(|| unsafe {
        SubclassingAdapter::new(
            view,
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
    .map_err(|_| NativeAccessibilityMountFailure::new(PLATFORM, ADAPTER_MOUNT_FAILURE))?;

    let (session, handle) =
        NativeAccessibilitySession::new(Box::new(IosTarget { adapter }), base_dpr, wake);
    let pending_activation = {
        let mut slot = handle_slot
            .lock()
            .expect("iOS accessibility callback slot poisoned");
        slot.handle = Some(handle.clone());
        std::mem::take(&mut slot.pending_activation)
    };
    if pending_activation {
        handle.activate();
    }
    Ok(session)
}
