use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use accesskit::{Action, ActionRequest, Affine, TreeId, TreeUpdate};
use hayate_app_host::{
    AppHost, DeliverySink, HeadlessPresentTarget, NativeAccessibilityDelivery,
    NativeAccessibilityMountFailure, NativeAccessibilitySession, NativeAccessibilityState,
    NativeAccessibilityTarget,
};
use hayate_core::{DocumentEventKind, ElementKind, ElementTree, EventDelivery};

struct RecordingTarget {
    updates: Arc<Mutex<Vec<TreeUpdate>>>,
}

struct SequenceTarget {
    updates: Arc<Mutex<Vec<TreeUpdate>>>,
    results: Arc<Mutex<VecDeque<NativeAccessibilityDelivery>>>,
}

impl NativeAccessibilityTarget for SequenceTarget {
    fn update(&mut self, update: TreeUpdate) -> NativeAccessibilityDelivery {
        self.updates.lock().unwrap().push(update);
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(NativeAccessibilityDelivery::Applied)
    }
}

impl NativeAccessibilityTarget for RecordingTarget {
    fn update(&mut self, update: TreeUpdate) -> NativeAccessibilityDelivery {
        self.updates.lock().unwrap().push(update);
        NativeAccessibilityDelivery::Applied
    }
}

#[test]
fn activation_wakes_and_delivers_initial_tree_after_the_next_frame() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let wake_counter = wakes.clone();
    let mut app = AppHost::new(
        HeadlessPresentTarget,
        Box::new(move || {
            wake_counter.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let root = app.tree_mut().element_create(10_000, ElementKind::View);
    let button = app.tree_mut().element_create(20_000, ElementKind::Button);
    app.tree_mut().element_append_child(root, button);
    app.tree_mut().set_root(root);

    let updates = Arc::new(Mutex::new(Vec::new()));
    let callbacks = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        2.0,
    );

    callbacks.activate();
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert!(updates.lock().unwrap().is_empty());

    app.tick(16.0).unwrap();

    let updates = updates.lock().unwrap();
    assert_eq!(updates.len(), 1);
    let initial = &updates[0];
    let root_id = initial.tree.as_ref().expect("full initial tree").root;
    let root_node = initial
        .nodes
        .iter()
        .find(|(id, _)| *id == root_id)
        .map(|(_, node)| node)
        .expect("root node");
    assert_eq!(root_node.transform(), Some(&Affine::scale(2.0)));
}

struct ClickMutationSink {
    button: hayate_core::ElementId,
}

impl DeliverySink for ClickMutationSink {
    fn handle(&mut self, deliveries: &[EventDelivery], tree: &mut ElementTree) {
        if !deliveries.is_empty() {
            tree.element_set_aria_label(self.button, "Clicked");
        }
    }
}

#[test]
fn action_callback_reaches_delivery_and_same_frame_outbound_state() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let wake_counter = wakes.clone();
    let mut app = AppHost::new(
        HeadlessPresentTarget,
        Box::new(move || {
            wake_counter.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let root = app.tree_mut().element_create(100, ElementKind::View);
    let button = app.tree_mut().element_create(900, ElementKind::Button);
    app.tree_mut().element_set_aria_label(button, "Ready");
    app.tree_mut().element_append_child(root, button);
    app.tree_mut().set_root(root);
    app.tree_mut()
        .register_listener(button, DocumentEventKind::Click);
    app.mount(Box::new(ClickMutationSink { button }));

    let updates = Arc::new(Mutex::new(Vec::new()));
    let callbacks = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        1.0,
    );
    callbacks.activate();
    app.tick(0.0).unwrap();
    let button_node = updates.lock().unwrap()[0]
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Ready"))
        .map(|(id, _)| *id)
        .expect("button node");

    callbacks.action(ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: button_node,
        data: None,
    });
    app.tick(16.0).unwrap();

    let updates = updates.lock().unwrap();
    assert_eq!(updates.len(), 2);
    assert!(
        updates[1]
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Clicked")),
        "action drain → delivery → consumer mutation → commit/present → outbound",
    );
}

#[test]
fn active_session_skips_unchanged_frames_and_sends_only_changed_nodes() {
    let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    let root = app.tree_mut().element_create(1, ElementKind::View);
    let button = app.tree_mut().element_create(2, ElementKind::Button);
    app.tree_mut().element_set_aria_label(button, "Before");
    app.tree_mut().element_append_child(root, button);
    app.tree_mut().set_root(root);

    let updates = Arc::new(Mutex::new(Vec::new()));
    let callbacks = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        1.0,
    );
    callbacks.activate();
    app.tick(0.0).unwrap();
    app.tick(16.0).unwrap();
    assert_eq!(updates.lock().unwrap().len(), 1);

    app.tree_mut().element_set_aria_label(button, "After");
    app.tick(32.0).unwrap();

    let updates = updates.lock().unwrap();
    assert_eq!(updates.len(), 2);
    let incremental = &updates[1];
    assert!(incremental.tree.is_none());
    assert_eq!(incremental.nodes.len(), 1);
    assert_eq!(incremental.nodes[0].1.label(), Some("After"));
}

#[test]
fn deactivation_ignores_actions_reactivation_is_full_and_close_detaches() {
    let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::Detached
    );
    let root = app.tree_mut().element_create(1, ElementKind::View);
    let button = app.tree_mut().element_create(2, ElementKind::Button);
    app.tree_mut().element_set_aria_label(button, "Ready");
    app.tree_mut().element_append_child(root, button);
    app.tree_mut().set_root(root);
    app.tree_mut()
        .register_listener(button, DocumentEventKind::Click);
    app.mount(Box::new(ClickMutationSink { button }));

    let updates = Arc::new(Mutex::new(Vec::new()));
    let callbacks = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        1.0,
    );
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::Dormant
    );
    callbacks.activate();
    app.tick(0.0).unwrap();
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::Active
    );
    let button_node = updates.lock().unwrap()[0]
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Ready"))
        .map(|(id, _)| *id)
        .expect("button node");

    callbacks.deactivate();
    callbacks.action(ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: button_node,
        data: None,
    });
    app.tick(16.0).unwrap();
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::Dormant
    );
    assert!(app
        .tree()
        .accessibility_update()
        .expect("current tree")
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("Ready")));
    assert_eq!(updates.lock().unwrap().len(), 1);

    callbacks.activate();
    app.tick(32.0).unwrap();
    assert!(updates.lock().unwrap()[1].tree.is_some());

    callbacks.close();
    app.tick(48.0).unwrap();
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::Detached
    );
}

#[test]
fn competing_activation_requests_each_receive_a_full_tree() {
    let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    let root = app.tree_mut().element_create(1, ElementKind::View);
    app.tree_mut().set_root(root);
    let updates = Arc::new(Mutex::new(Vec::new()));
    let callbacks = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        1.0,
    );

    callbacks.activate();
    callbacks.activate();
    app.tick(0.0).unwrap();

    let updates = updates.lock().unwrap();
    assert_eq!(updates.len(), 2);
    assert!(updates.iter().all(|update| update.tree.is_some()));
}

#[test]
fn mount_failure_disables_only_accessibility_and_remains_observable() {
    let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    let root = app.tree_mut().element_create(1, ElementKind::View);
    app.tree_mut().set_root(root);
    let failure = NativeAccessibilityMountFailure::new("android", "adapter-init");

    let callbacks = app.try_mount_native_accessibility(Err(failure.clone()), 3.0);

    assert!(callbacks.is_none());
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::Disabled
    );
    assert_eq!(app.native_accessibility_mount_failure(), Some(&failure));
    app.tick(0.0)
        .expect("rendering continues without accessibility");
}

#[test]
fn base_dpr_change_updates_only_the_root_transform() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let wake_counter = wakes.clone();
    let mut app = AppHost::new(
        HeadlessPresentTarget,
        Box::new(move || {
            wake_counter.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let root = app.tree_mut().element_create(1, ElementKind::View);
    let child = app.tree_mut().element_create(2, ElementKind::Button);
    app.tree_mut().element_append_child(root, child);
    app.tree_mut().set_root(root);
    let updates = Arc::new(Mutex::new(Vec::new()));
    let callbacks = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        1.0,
    );
    callbacks.activate();
    app.tick(0.0).unwrap();

    app.set_native_accessibility_base_dpr(2.5);
    app.tick(16.0).unwrap();

    let updates = updates.lock().unwrap();
    let scale_update = &updates[1];
    assert!(scale_update.tree.is_none());
    assert_eq!(scale_update.nodes.len(), 1);
    assert_eq!(
        scale_update.nodes[0].1.transform(),
        Some(&Affine::scale(2.5))
    );
    assert!(wakes.load(Ordering::SeqCst) >= 2);
}

#[test]
fn inactive_delivery_does_not_advance_baseline_or_retry_blindly() {
    let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    let root = app.tree_mut().element_create(1, ElementKind::View);
    let button = app.tree_mut().element_create(2, ElementKind::Button);
    app.tree_mut().element_set_aria_label(button, "Before");
    app.tree_mut().element_append_child(root, button);
    app.tree_mut().set_root(root);
    let updates = Arc::new(Mutex::new(Vec::new()));
    let results = Arc::new(Mutex::new(VecDeque::from([
        NativeAccessibilityDelivery::Inactive,
        NativeAccessibilityDelivery::Applied,
    ])));
    let callbacks = app.mount_native_accessibility(
        Box::new(SequenceTarget {
            updates: updates.clone(),
            results,
        }),
        1.0,
    );

    callbacks.activate();
    app.tick(0.0).unwrap();
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::Dormant
    );
    app.tree_mut().element_set_aria_label(button, "After");
    app.tick(16.0).unwrap();
    assert_eq!(updates.lock().unwrap().len(), 1);

    callbacks.activate();
    app.tick(32.0).unwrap();
    let updates = updates.lock().unwrap();
    assert_eq!(updates.len(), 2);
    assert!(updates[1].tree.is_some());
    assert!(updates[1]
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("After")));
}

#[test]
fn action_arriving_during_initial_pending_is_reflected_in_initial_focus() {
    let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    let root = app.tree_mut().element_create(1, ElementKind::View);
    let input = app.tree_mut().element_create(2, ElementKind::TextInput);
    app.tree_mut().element_append_child(root, input);
    app.tree_mut().set_root(root);
    app.tick(0.0).unwrap();
    let input_node = app
        .tree()
        .accessibility_update()
        .expect("laid out tree")
        .nodes
        .into_iter()
        .find(|(_, node)| node.role() == accesskit::Role::TextInput)
        .map(|(id, _)| id)
        .expect("input node");

    let updates = Arc::new(Mutex::new(Vec::new()));
    let callbacks = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        1.0,
    );
    callbacks.activate();
    callbacks.action(ActionRequest {
        action: Action::Focus,
        target_tree: TreeId::ROOT,
        target_node: input_node,
        data: None,
    });

    let prepared = app.prepare_frame(16.0).unwrap();
    assert_eq!(
        app.native_accessibility_state(),
        NativeAccessibilityState::InitialPending
    );
    app.commit_frame(prepared.frame_id()).unwrap();

    let updates = updates.lock().unwrap();
    assert_eq!(updates[0].focus, input_node);
}

#[test]
fn replacing_the_target_discards_the_old_baseline() {
    let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    let root = app.tree_mut().element_create(1, ElementKind::View);
    app.tree_mut().set_root(root);
    let first_updates = Arc::new(Mutex::new(Vec::new()));
    let first = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: first_updates.clone(),
        }),
        1.0,
    );
    first.activate();
    app.tick(0.0).unwrap();

    let replacement_updates = Arc::new(Mutex::new(Vec::new()));
    let replacement = app.mount_native_accessibility(
        Box::new(RecordingTarget {
            updates: replacement_updates.clone(),
        }),
        1.0,
    );
    replacement.activate();
    app.tick(16.0).unwrap();

    let replacement_updates = replacement_updates.lock().unwrap();
    assert_eq!(replacement_updates.len(), 1);
    assert!(replacement_updates[0].tree.is_some());
    assert!(!replacement_updates[0].nodes.is_empty());
}

#[test]
fn replacing_a_tree_on_the_same_surface_requests_a_new_full_baseline() {
    let updates = Arc::new(Mutex::new(Vec::new()));
    let (mut session, callbacks) = NativeAccessibilitySession::new(
        Box::new(RecordingTarget {
            updates: updates.clone(),
        }),
        1.0,
        Arc::new(|| {}),
    );
    let mut first_tree = ElementTree::new();
    let first_root = first_tree.element_create(1, ElementKind::View);
    first_tree.element_set_aria_label(first_root, "first");
    first_tree.set_root(first_root);
    callbacks.activate();
    session.drain_before_frame(&mut first_tree);
    first_tree.commit_rendered_frame(0.0);
    session.update_after_present(&first_tree);

    let mut replacement_tree = ElementTree::new();
    let replacement_root = replacement_tree.element_create(2, ElementKind::View);
    replacement_tree.element_set_aria_label(replacement_root, "replacement");
    replacement_tree.set_root(replacement_root);
    session.reset_for_tree_replacement();
    replacement_tree.commit_rendered_frame(16.0);
    session.update_after_present(&replacement_tree);

    let updates = updates.lock().unwrap();
    assert_eq!(updates.len(), 2);
    assert!(updates[1].tree.is_some());
    assert!(updates[1]
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("replacement")));
}
