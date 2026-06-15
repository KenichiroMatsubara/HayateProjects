//! AccessKit tree generation from ElementTree (ADR-0041).

use accesskit::{Action, ActionRequest, Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate};

use super::taffy_projection::TraversalStep;
use super::tree::Element;
use super::{ElementId, ElementKind, ElementTree};

fn node_id(id: ElementId) -> NodeId {
    NodeId(id.to_u64())
}

/// Core-owned, supported subset of inbound AccessKit actions (ADR-0098).
///
/// AccessKit's `Action` is a wide protocol vocabulary; Core maps it down to the
/// operations it actually drives and folds everything else to `Ignored`, so the
/// inbound surface is total and the runtime never sees a native-only concept.
/// The mapping lives entirely in Core (Rust API) and is never put on the proto
/// wire (ADR-0098 Decision 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityAction {
    /// Move focus to `target`, driving the existing focus state machine.
    Focus { target: ElementId },
    /// Unsupported action — no-op, observable state unchanged.
    Ignored,
}

/// Pure mapping from an AccessKit `ActionRequest` to the Core action subset
/// (ADR-0098 Decision 3). Inbound `NodeId` is resolved element-only for v1 as
/// the inverse of outbound `NodeId(ElementId.to_u64())`, i.e. `ElementId::from_u64`.
pub fn map_action_request(req: &ActionRequest) -> AccessibilityAction {
    let target = ElementId::from_u64(req.target_node.0);
    match req.action {
        Action::Focus => AccessibilityAction::Focus { target },
        _ => AccessibilityAction::Ignored,
    }
}

fn aria_role(role: &str) -> Option<Role> {
    match role {
        "button" => Some(Role::Button),
        "label" => Some(Role::Label),
        "text-input" | "textbox" => Some(Role::TextInput),
        "scroll-view" => Some(Role::ScrollView),
        "image" | "img" => Some(Role::Image),
        "list" => Some(Role::List),
        "list-item" | "listitem" => Some(Role::ListItem),
        "heading" => Some(Role::Heading),
        "link" => Some(Role::Link),
        "navigation" => Some(Role::Navigation),
        "main" => Some(Role::Main),
        "dialog" => Some(Role::Dialog),
        "alert-dialog" => Some(Role::AlertDialog),
        "generic-container" => Some(Role::GenericContainer),
        _ => None,
    }
}

fn implicit_role(kind: ElementKind) -> Role {
    match kind {
        ElementKind::View => Role::GenericContainer,
        ElementKind::Text => Role::Label,
        ElementKind::Image => Role::Image,
        ElementKind::Button => Role::Button,
        ElementKind::TextInput => Role::TextInput,
        ElementKind::ScrollView => Role::ScrollView,
    }
}

fn resolve_role(el: &Element, is_root: bool) -> Role {
    if is_root {
        return Role::Window;
    }
    if let Some(role) = el.role.as_deref().and_then(aria_role) {
        return role;
    }
    implicit_role(el.kind)
}

fn element_value(el: &Element) -> Option<String> {
    match el.kind {
        ElementKind::Text => el.text.clone(),
        ElementKind::TextInput => el.edit.as_ref().map(|edit| edit.display_text()),
        ElementKind::Button => el.text.clone(),
        _ => None,
    }
}

fn build_node(el: &Element, bounds: (f32, f32, f32, f32), is_root: bool) -> Node {
    let (x, y, w, h) = bounds;
    let mut node = Node::new(resolve_role(el, is_root));
    node.set_bounds(Rect {
        x0: x as f64,
        y0: y as f64,
        x1: (x + w) as f64,
        y1: (y + h) as f64,
    });
    if let Some(label) = &el.aria_label {
        node.set_label(label.clone());
    }
    if let Some(value) = element_value(el) {
        node.set_value(value);
    }
    node
}

/// Walk the Canonical Tree building AccessKit nodes, returning the ids of the
/// top-level nodes produced for `id`'s subtree (so the caller can attach them
/// as children).
///
/// Elements with no Taffy node (e.g. inline text elements inside an IFC) are
/// skipped but their children are still recursed into and their top-level
/// nodes bubble up to the nearest ancestor with a Taffy node — this is what
/// fixes the IFC subtree drop.
fn walk_accessibility(
    tree: &ElementTree,
    id: ElementId,
    root_id: ElementId,
    nodes: &mut Vec<(NodeId, Node)>,
) -> Vec<NodeId> {
    let step = match tree.layout.projection.traversal_step(&tree.elements, id) {
        Some(step) => step,
        None => return Vec::new(),
    };

    let el = match step {
        TraversalStep::Skip(el) => {
            let mut top_ids = Vec::new();
            for &child in &el.children {
                top_ids.extend(walk_accessibility(tree, child, root_id, nodes));
            }
            return top_ids;
        }
        TraversalStep::Visit(_, el) => el,
    };

    let Some(&(x, y, w, h)) = tree.layout.layout_cache.get(&id) else {
        return Vec::new();
    };

    let mut node = build_node(el, (x, y, w, h), id == root_id);
    let this_id = node_id(id);

    for &child in &el.children {
        for child_id in walk_accessibility(tree, child, root_id, nodes) {
            node.push_child(child_id);
        }
    }

    nodes.push((this_id, node));
    vec![this_id]
}

impl ElementTree {
    /// Inbound AccessKit action surface (ADR-0098): the mirror of outbound
    /// `accessibility_update`. Platform Adapters bridge an AT request here and
    /// Core maps it to an existing runtime intent — never a synthetic pointer or
    /// key replay (Flutter-style semantic action). Unsupported actions fold to
    /// `Ignored` and are no-ops.
    pub fn on_accessibility_action(&mut self, req: ActionRequest) {
        match map_action_request(&req) {
            AccessibilityAction::Focus { target } => self.transition_focus(target),
            AccessibilityAction::Ignored => {}
        }
    }

    /// Build an AccessKit `TreeUpdate` from the current element tree and layout cache.
    ///
    /// Returns `None` when layout has not run or the tree has no root.
    pub fn accessibility_update(&self) -> Option<TreeUpdate> {
        let root_id = self.root?;
        if self.layout.layout_cache.is_empty() {
            return None;
        }

        let mut nodes = Vec::new();
        walk_accessibility(self, root_id, root_id, &mut nodes);

        let focus = self
            .focused_element
            .map(node_id)
            .unwrap_or_else(|| node_id(root_id));

        Some(TreeUpdate {
            nodes,
            tree: Some(Tree::new(node_id(root_id))),
            tree_id: TreeId::ROOT,
            focus,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{DisplayValue, DocumentEventKind, StyleProp};
    use accesskit::{Action, ActionRequest};

    fn action_request(action: Action, node: NodeId) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: None,
        }
    }

    #[test]
    fn maps_focus_action_to_core_focus_resolving_element_id() {
        let target = ElementId::from_u64(7);
        let mapped = map_action_request(&action_request(Action::Focus, node_id(target)));
        assert_eq!(mapped, AccessibilityAction::Focus { target });
    }

    #[test]
    fn folds_unsupported_actions_to_ignored() {
        let node = node_id(ElementId::from_u64(3));
        for action in [
            Action::Increment,
            Action::ShowContextMenu,
            Action::CustomAction,
        ] {
            assert_eq!(
                map_action_request(&action_request(action, node)),
                AccessibilityAction::Ignored,
                "{action:?} should fold to Ignored"
            );
        }
    }

    #[test]
    fn on_accessibility_action_focus_drives_focus_state_machine() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let a = tree.element_create(2, ElementKind::TextInput);
        let b = tree.element_create(3, ElementKind::TextInput);
        tree.element_append_child(root, a);
        tree.element_append_child(root, b);

        let la_focus = tree.register_listener(a, DocumentEventKind::Focus);
        let la_blur = tree.register_listener(a, DocumentEventKind::Blur);
        let lb_focus = tree.register_listener(b, DocumentEventKind::Focus);

        tree.on_accessibility_action(action_request(Action::Focus, node_id(a)));
        assert_eq!(tree.focused_element(), Some(a));
        let first: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert_eq!(first, vec![la_focus]);

        // Focusing b must blur the previously focused a, then focus b.
        tree.on_accessibility_action(action_request(Action::Focus, node_id(b)));
        assert_eq!(tree.focused_element(), Some(b));
        let second: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert_eq!(second, vec![la_blur, lb_focus]);
    }

    #[test]
    fn on_accessibility_action_ignores_unsupported_action() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let a = tree.element_create(2, ElementKind::TextInput);
        let b = tree.element_create(3, ElementKind::Button);
        tree.element_append_child(root, a);
        tree.element_append_child(root, b);
        tree.register_listener(b, DocumentEventKind::Focus);

        tree.on_accessibility_action(action_request(Action::Focus, node_id(a)));
        let _ = tree.poll_deliveries();

        // An unsupported action targeting b must not move focus or emit anything.
        tree.on_accessibility_action(action_request(Action::Increment, node_id(b)));
        assert_eq!(tree.focused_element(), Some(a));
        assert!(tree.poll_deliveries().is_empty());
    }

    #[test]
    fn accessibility_update_includes_bounds_and_roles() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.element_set_aria_label(button, "Confirm");
        tree.element_set_role(button, "button");
        let input = tree.element_create(3, ElementKind::TextInput);
        tree.element_append_child(root, input);
        tree.element_set_text_content(input, "hello");
        tree.render(0.0);

        let update = tree.accessibility_update().expect("tree update");
        assert_eq!(update.tree_id, TreeId::ROOT);
        assert_eq!(update.focus, node_id(root));
        assert!(update.nodes.len() >= 3);

        let button_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id(button))
            .map(|(_, n)| n)
            .expect("button node");
        assert_eq!(button_node.role(), Role::Button);
        assert_eq!(button_node.label(), Some("Confirm"));

        let input_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id(input))
            .map(|(_, n)| n)
            .expect("input node");
        assert_eq!(input_node.role(), Role::TextInput);
        assert_eq!(input_node.value(), Some("hello"));
    }

    #[test]
    fn accessibility_update_does_not_drop_ifc_inline_text_subtree() {
        use std::collections::HashSet;

        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);

        // IFC root: a `text` element under a non-text parent.
        let ifc_root = tree.element_create(2, ElementKind::Text);
        tree.element_append_child(root, ifc_root);
        tree.element_set_text(ifc_root, "Hello ");

        // Inline text element: a `text` element under a `text` parent — has no
        // Taffy node (ADR-0063/0064).
        let inline = tree.element_create(3, ElementKind::Text);
        tree.element_append_child(ifc_root, inline);
        tree.element_set_text(inline, "world");

        tree.render(0.0);

        let update = tree.accessibility_update().expect("tree update");

        // The IFC root itself must still be present in the AccessKit tree.
        assert!(
            update.nodes.iter().any(|(id, _)| *id == node_id(ifc_root)),
            "IFC root subtree was dropped from the AccessKit tree"
        );

        // No node may reference a child id that has no corresponding node —
        // that would indicate a dropped subtree.
        let node_ids: HashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();
        for (_, node) in &update.nodes {
            for child in node.children() {
                assert!(
                    node_ids.contains(child),
                    "dangling child reference: {child:?}"
                );
            }
        }
    }
}
