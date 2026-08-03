//! Android `AccessibilityNodeProvider` target for the shared native accessibility session.
//!
//! Baseline, activation, action ordering, and root DPR transform remain owned by
//! `NativeAccessibilitySession`. This leaf only retains enough AccessKit nodes to project an
//! Android virtual-view snapshot and crosses JNI through `jni_bridge`.

use accesskit::{Action, ActionData, ActionRequest, NodeId, Role, TreeId, TreeUpdate};
use accesskit_consumer::{Node as ConsumerNode, Tree as ConsumerTree, TreeChangeHandler};
use hayate_app_host::{NativeAccessibilityDelivery, NativeAccessibilityTarget};
#[cfg(target_os = "android")]
use hayate_app_host::{
    NativeAccessibilityHandle, NativeAccessibilityMountFailure, NativeAccessibilitySession,
};
use serde::Serialize;

const PLATFORM: &str = "android";
#[cfg(target_os = "android")]
const ADAPTER_MOUNT_FAILURE: &str = "adapter-mount";
const TARGET_DELIVERY_FAILURE: &str = "target-delivery";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidTreeSnapshot {
    root_id: String,
    focus_id: String,
    nodes: Vec<AndroidNodeSnapshot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidNodeSnapshot {
    id: String,
    parent_id: Option<String>,
    children: Vec<String>,
    class_name: &'static str,
    label: Option<String>,
    value: Option<String>,
    bounds: [i32; 4],
    disabled: bool,
    actions: Vec<&'static str>,
}

trait AndroidAccessibilityPort {
    fn update(&mut self, snapshot_json: &str) -> Result<bool, String>;
}

struct AndroidTarget {
    port: Box<dyn AndroidAccessibilityPort>,
    tree: Option<ConsumerTree>,
}

impl AndroidTarget {
    fn new(port: Box<dyn AndroidAccessibilityPort>) -> Self {
        Self { port, tree: None }
    }

    fn apply(&mut self, update: TreeUpdate) -> Result<String, String> {
        if let Some(tree) = self.tree.as_mut() {
            tree.update_and_process_changes(update, &mut IgnoreConsumerChanges);
        } else if update.tree.is_some() {
            self.tree = Some(ConsumerTree::new(update, true));
        } else {
            return Err("initial update has no root".to_owned());
        }
        let state = self
            .tree
            .as_ref()
            .expect("consumer tree initialized")
            .state();
        let root = state.root();
        let mut nodes = Vec::new();
        self.project_subtree(root, None, &mut nodes);
        serde_json::to_string(&AndroidTreeSnapshot {
            root_id: node_id_string(root),
            focus_id: node_id_string(state.focus_in_tree()),
            nodes,
        })
        .map_err(|error| error.to_string())
    }

    fn project_subtree(
        &self,
        node: ConsumerNode<'_>,
        parent: Option<ConsumerNode<'_>>,
        out: &mut Vec<AndroidNodeSnapshot>,
    ) {
        let children: Vec<_> = node.children().collect();
        let bounds = node
            .bounding_box()
            .map(|bounds| {
                [
                    saturating_i32(bounds.x0.floor()),
                    saturating_i32(bounds.y0.floor()),
                    saturating_i32(bounds.x1.ceil()),
                    saturating_i32(bounds.y1.ceil()),
                ]
            })
            .unwrap_or([0, 0, 0, 0]);
        let mut actions = Vec::new();
        for (action, name) in [
            (Action::Focus, "focus"),
            (Action::Click, "click"),
            (Action::SetValue, "setValue"),
            (Action::ScrollIntoView, "scrollIntoView"),
        ] {
            if node.data().supports_action(action) {
                actions.push(name);
            }
        }
        out.push(AndroidNodeSnapshot {
            id: node_id_string(node),
            parent_id: parent.map(node_id_string),
            children: children.iter().copied().map(node_id_string).collect(),
            class_name: android_class(node.role()),
            label: node.label(),
            value: node.value(),
            bounds,
            disabled: node.is_disabled(),
            actions,
        });
        for child in children {
            self.project_subtree(child, Some(node), out);
        }
    }
}

struct IgnoreConsumerChanges;

impl TreeChangeHandler for IgnoreConsumerChanges {
    fn node_added(&mut self, _node: &ConsumerNode<'_>) {}
    fn node_updated(&mut self, _old_node: &ConsumerNode<'_>, _new_node: &ConsumerNode<'_>) {}
    fn focus_moved(
        &mut self,
        _old_node: Option<&ConsumerNode<'_>>,
        _new_node: Option<&ConsumerNode<'_>>,
    ) {
    }
    fn node_removed(&mut self, _node: &ConsumerNode<'_>) {}
}

impl NativeAccessibilityTarget for AndroidTarget {
    fn update(&mut self, update: TreeUpdate) -> NativeAccessibilityDelivery {
        let result = self
            .apply(update)
            .and_then(|snapshot| self.port.update(&snapshot));
        match result {
            Ok(true) => NativeAccessibilityDelivery::Applied,
            Ok(false) => NativeAccessibilityDelivery::Inactive,
            Err(error) => {
                log::error!(
                    "native-accessibility platform={PLATFORM} category={TARGET_DELIVERY_FAILURE}: {error}"
                );
                NativeAccessibilityDelivery::Inactive
            }
        }
    }
}

fn node_id_string(node: ConsumerNode<'_>) -> String {
    u64::from(node.locate().0).to_string()
}

fn saturating_i32(value: f64) -> i32 {
    value.clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

fn android_class(role: Role) -> &'static str {
    match role {
        Role::Button | Role::DefaultButton => "android.widget.Button",
        Role::TextInput
        | Role::MultilineTextInput
        | Role::SearchInput
        | Role::EmailInput
        | Role::NumberInput
        | Role::PasswordInput
        | Role::PhoneNumberInput
        | Role::UrlInput => "android.widget.EditText",
        Role::Image => "android.widget.ImageView",
        Role::List => "android.widget.ListView",
        Role::ListItem => "android.view.ViewGroup",
        Role::Label | Role::Paragraph | Role::TextRun => "android.widget.TextView",
        _ => "android.view.View",
    }
}

pub(crate) fn action_request(
    action: &str,
    node_id: u64,
    value: Option<String>,
) -> Option<ActionRequest> {
    let (action, data) = match action {
        "focus" => (Action::Focus, None),
        "click" => (Action::Click, None),
        "setValue" => (
            Action::SetValue,
            Some(ActionData::Value(value?.into_boxed_str())),
        ),
        "scrollIntoView" => (Action::ScrollIntoView, None),
        _ => return None,
    };
    Some(ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: NodeId(node_id),
        data,
    })
}

#[cfg(target_os = "android")]
struct JniAndroidPort;

#[cfg(target_os = "android")]
impl AndroidAccessibilityPort for JniAndroidPort {
    fn update(&mut self, snapshot_json: &str) -> Result<bool, String> {
        crate::jni_bridge::android_accessibility_update(snapshot_json)
    }
}

#[cfg(target_os = "android")]
fn callback_slot() -> &'static std::sync::Mutex<Option<NativeAccessibilityHandle>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<NativeAccessibilityHandle>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(target_os = "android")]
pub(crate) fn mount_android_accessibility(
    base_dpr: f64,
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
) -> Result<NativeAccessibilitySession, NativeAccessibilityMountFailure> {
    crate::jni_bridge::android_accessibility_mount().map_err(|error| {
        log::error!(
            "native-accessibility platform={PLATFORM} category={ADAPTER_MOUNT_FAILURE}: {error}"
        );
        NativeAccessibilityMountFailure::new(PLATFORM, ADAPTER_MOUNT_FAILURE)
    })?;
    let (session, handle) = NativeAccessibilitySession::new(
        Box::new(AndroidTarget::new(Box::new(JniAndroidPort))),
        base_dpr,
        wake,
    );
    *callback_slot()
        .lock()
        .expect("android accessibility callback slot poisoned") = Some(handle);
    Ok(session)
}

#[cfg(target_os = "android")]
pub(crate) fn destroy_android_accessibility(session: &mut Option<NativeAccessibilitySession>) {
    if let Some(handle) = callback_slot()
        .lock()
        .expect("android accessibility callback slot poisoned")
        .take()
    {
        handle.close();
    }
    *session = None;
    if let Err(error) = crate::jni_bridge::android_accessibility_unmount() {
        log::error!(
            "native-accessibility platform={PLATFORM} category={TARGET_DELIVERY_FAILURE}: {error}"
        );
    }
}

#[cfg(target_os = "android")]
pub(crate) fn activate_callback() {
    with_callback(NativeAccessibilityHandle::activate);
}

#[cfg(target_os = "android")]
pub(crate) fn deactivate_callback() {
    with_callback(NativeAccessibilityHandle::deactivate);
}

#[cfg(target_os = "android")]
pub(crate) fn action_callback(request: ActionRequest) {
    with_callback(|handle| handle.action(request));
}

#[cfg(target_os = "android")]
fn with_callback(callback: impl FnOnce(&NativeAccessibilityHandle)) {
    if let Some(handle) = callback_slot()
        .lock()
        .expect("android accessibility callback slot poisoned")
        .as_ref()
    {
        callback(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Affine, Node, Rect, Tree};
    use std::sync::{Arc, Mutex};

    struct RecordingPort(Arc<Mutex<Vec<String>>>);

    impl AndroidAccessibilityPort for RecordingPort {
        fn update(&mut self, json: &str) -> Result<bool, String> {
            self.0.lock().unwrap().push(json.to_owned());
            Ok(true)
        }
    }

    #[test]
    fn projects_accesskit_actions_and_root_density_into_android_snapshot() {
        let root_id = NodeId(1);
        let button_id = NodeId(2);
        let mut root = Node::new(Role::Window);
        root.set_children(vec![button_id]);
        root.set_transform(Affine::scale(2.0));
        let mut button = Node::new(Role::Button);
        button.set_label("Save");
        button.set_bounds(Rect::new(1.0, 2.0, 11.0, 12.0));
        button.add_action(Action::Focus);
        button.add_action(Action::Click);
        button.add_action(Action::ScrollIntoView);
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut target = AndroidTarget::new(Box::new(RecordingPort(output.clone())));

        assert_eq!(
            target.update(TreeUpdate {
                nodes: vec![(root_id, root), (button_id, button)],
                tree: Some(Tree::new(root_id)),
                tree_id: TreeId::ROOT,
                focus: button_id,
            }),
            NativeAccessibilityDelivery::Applied
        );

        let snapshot: serde_json::Value = serde_json::from_str(&output.lock().unwrap()[0]).unwrap();
        assert_eq!(snapshot["focusId"], "2");
        assert_eq!(
            snapshot["nodes"][1]["bounds"],
            serde_json::json!([2, 4, 22, 24])
        );
        assert_eq!(
            snapshot["nodes"][1]["actions"],
            serde_json::json!(["focus", "click", "scrollIntoView"])
        );
    }

    #[test]
    fn maps_only_the_supported_android_action_subset() {
        assert_eq!(
            action_request("focus", 7, None).unwrap().action,
            Action::Focus
        );
        assert_eq!(
            action_request("click", 7, None).unwrap().action,
            Action::Click
        );
        assert!(matches!(
            action_request("setValue", 7, Some("hello".to_owned())).unwrap().data,
            Some(ActionData::Value(value)) if value.as_ref() == "hello"
        ));
        assert_eq!(
            action_request("scrollIntoView", 7, None).unwrap().action,
            Action::ScrollIntoView
        );
        assert!(action_request("increment", 7, None).is_none());
    }
}
