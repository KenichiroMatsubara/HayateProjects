use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use accesskit::{ActionRequest, Affine, Node, NodeId, TreeUpdate};
use hayate_core::ElementTree;

/// Native target が一つの update を受理したかを表す lifecycle 結果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAccessibilityDelivery {
    Applied,
    Inactive,
}

/// Platform Front が mount する OS 固有 target の最小 seam。
pub trait NativeAccessibilityTarget {
    fn update(&mut self, update: TreeUpdate) -> NativeAccessibilityDelivery;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAccessibilityState {
    Detached,
    Disabled,
    Dormant,
    InitialPending,
    Active,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAccessibilityMountFailure {
    pub platform: Box<str>,
    pub category: Box<str>,
}

impl NativeAccessibilityMountFailure {
    pub fn new(platform: impl Into<Box<str>>, category: impl Into<Box<str>>) -> Self {
        Self {
            platform: platform.into(),
            category: category.into(),
        }
    }
}

enum MailboxMessage {
    Activate,
    Deactivate,
    Action(ActionRequest),
    Close,
}

struct Mailbox {
    messages: Mutex<VecDeque<MailboxMessage>>,
}

/// OS callback へ渡す clone 可能な入口。ElementTree を持たず、mailbox と wake だけを持つ。
#[derive(Clone)]
pub struct NativeAccessibilityHandle {
    mailbox: Arc<Mailbox>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl NativeAccessibilityHandle {
    pub fn activate(&self) {
        self.enqueue(MailboxMessage::Activate);
    }

    pub fn action(&self, request: ActionRequest) {
        self.enqueue(MailboxMessage::Action(request));
    }

    pub fn deactivate(&self) {
        self.enqueue(MailboxMessage::Deactivate);
    }

    pub fn close(&self) {
        self.enqueue(MailboxMessage::Close);
    }

    fn enqueue(&self, message: MailboxMessage) {
        self.mailbox
            .messages
            .lock()
            .expect("native accessibility mailbox poisoned")
            .push_back(message);
        (self.wake)();
    }
}

/// Shared lifecycle/ordering engine used by AppHost and direct native platform fronts.
/// Platform adapters own only their OS target; this type remains the sole baseline/mailbox owner.
pub struct NativeAccessibilitySession {
    target: Option<Box<dyn NativeAccessibilityTarget>>,
    mailbox: Arc<Mailbox>,
    state: NativeAccessibilityState,
    base_dpr: f64,
    last_generation: Option<u64>,
    baseline: HashMap<NodeId, Node>,
    pending_full_updates: usize,
    scale_dirty: bool,
}

impl NativeAccessibilitySession {
    pub fn new(
        target: Box<dyn NativeAccessibilityTarget>,
        base_dpr: f64,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> (Self, NativeAccessibilityHandle) {
        let mailbox = Arc::new(Mailbox {
            messages: Mutex::new(VecDeque::new()),
        });
        let handle = NativeAccessibilityHandle {
            mailbox: mailbox.clone(),
            wake,
        };
        (
            Self {
                target: Some(target),
                mailbox,
                state: NativeAccessibilityState::Dormant,
                base_dpr,
                last_generation: None,
                baseline: HashMap::new(),
                pending_full_updates: 0,
                scale_dirty: true,
            },
            handle,
        )
    }

    pub fn drain_before_frame(&mut self, tree: &mut ElementTree) {
        let messages: Vec<_> = self
            .mailbox
            .messages
            .lock()
            .expect("native accessibility mailbox poisoned")
            .drain(..)
            .collect();
        for message in messages {
            match message {
                MailboxMessage::Activate => {
                    if self.target.is_some() {
                        self.state = NativeAccessibilityState::InitialPending;
                        self.last_generation = None;
                        self.baseline.clear();
                        self.pending_full_updates = self.pending_full_updates.saturating_add(1);
                    }
                }
                MailboxMessage::Deactivate => {
                    self.state = if self.target.is_some() {
                        NativeAccessibilityState::Dormant
                    } else {
                        NativeAccessibilityState::Detached
                    };
                    self.last_generation = None;
                    self.baseline.clear();
                    self.pending_full_updates = 0;
                }
                MailboxMessage::Action(request)
                    if matches!(
                        self.state,
                        NativeAccessibilityState::InitialPending | NativeAccessibilityState::Active
                    ) =>
                {
                    tree.on_accessibility_action(request);
                }
                MailboxMessage::Action(_) => {}
                MailboxMessage::Close => {
                    self.target = None;
                    self.state = NativeAccessibilityState::Detached;
                    self.last_generation = None;
                    self.baseline.clear();
                    self.pending_full_updates = 0;
                }
            }
        }
    }

    pub fn update_after_present(&mut self, tree: &ElementTree) {
        if matches!(
            self.state,
            NativeAccessibilityState::Detached
                | NativeAccessibilityState::Disabled
                | NativeAccessibilityState::Dormant
        ) {
            return;
        }
        let generation = tree.accessibility_generation();
        if self.state == NativeAccessibilityState::Active
            && self.last_generation == Some(generation)
            && !self.scale_dirty
        {
            return;
        }
        let full_update = self.state == NativeAccessibilityState::InitialPending;
        let Some(mut update) = tree.accessibility_update() else {
            return;
        };
        apply_root_scale(&mut update, self.base_dpr);
        let next_baseline: HashMap<_, _> = update.nodes.iter().cloned().collect();
        if !full_update {
            update.tree = None;
            update
                .nodes
                .retain(|(id, node)| self.baseline.get(id) != Some(node));
        }
        let Some(target) = self.target.as_mut() else {
            self.state = NativeAccessibilityState::Detached;
            return;
        };
        if full_update {
            let delivery_count = self.pending_full_updates.max(1);
            for _ in 0..delivery_count {
                if target.update(update.clone()) == NativeAccessibilityDelivery::Inactive {
                    self.state = NativeAccessibilityState::Dormant;
                    self.last_generation = None;
                    self.baseline.clear();
                    self.pending_full_updates = 0;
                    return;
                }
            }
            self.state = NativeAccessibilityState::Active;
            self.last_generation = Some(generation);
            self.baseline = next_baseline;
            self.pending_full_updates = 0;
            self.scale_dirty = false;
            return;
        }
        match target.update(update) {
            NativeAccessibilityDelivery::Applied => {
                self.state = NativeAccessibilityState::Active;
                self.last_generation = Some(generation);
                self.baseline = next_baseline;
                self.pending_full_updates = 0;
                self.scale_dirty = false;
            }
            NativeAccessibilityDelivery::Inactive => {
                self.state = NativeAccessibilityState::Dormant;
                self.last_generation = None;
                self.baseline.clear();
                self.pending_full_updates = 0;
                self.scale_dirty = false;
            }
        }
    }

    pub fn state(&self) -> NativeAccessibilityState {
        self.state
    }

    pub fn set_base_dpr(&mut self, base_dpr: f64) -> bool {
        if self.base_dpr == base_dpr {
            return false;
        }
        self.base_dpr = base_dpr;
        self.scale_dirty = true;
        true
    }

    /// A platform runtime replaced its ElementTree while retaining the same native surface.
    /// Active targets need a new full baseline; dormant targets wait for their next activation.
    pub fn reset_for_tree_replacement(&mut self) {
        self.last_generation = None;
        self.baseline.clear();
        self.pending_full_updates = 0;
        if self.state == NativeAccessibilityState::Active {
            self.state = NativeAccessibilityState::InitialPending;
            self.pending_full_updates = 1;
        }
    }
}

fn apply_root_scale(update: &mut TreeUpdate, base_dpr: f64) {
    let Some(root_id) = update.tree.as_ref().map(|tree| tree.root) else {
        return;
    };
    if let Some((_, root)) = update.nodes.iter_mut().find(|(id, _)| *id == root_id) {
        root.set_transform(Affine::scale(base_dpr));
    }
}
