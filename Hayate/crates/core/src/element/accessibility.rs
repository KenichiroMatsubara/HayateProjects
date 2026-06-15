//! AccessKit tree generation from ElementTree (ADR-0041).

use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate,
};

use super::taffy_projection::TraversalStep;
use super::tree::Element;
use super::{DocumentEventKind, ElementId, ElementKind, ElementTree, Event};

fn node_id(id: ElementId) -> NodeId {
    NodeId(id.to_u64())
}

/// Minimal new scroll offset along one axis so the target span
/// `[content_pos, content_pos + size]` becomes visible inside the viewport
/// `[offset, offset + viewport]`, clamped to `[0, max]`.
///
/// Returns the current `offset` unchanged when the target is already fully
/// visible (or already spans the whole viewport), so an already-visible target
/// never moves the offset. Otherwise it aligns the nearest edge: the leading
/// edge when the target sits before the viewport, the trailing edge when it
/// extends past it. Pure offset arithmetic — no inertia (ADR-0098 Decision 4).
fn scroll_axis_to_reveal(content_pos: f32, size: f32, viewport: f32, offset: f32, max: f32) -> f32 {
    let lead = content_pos;
    let trail = content_pos + size;
    let view_lead = offset;
    let view_trail = offset + viewport;
    let new_offset = if lead >= view_lead && trail <= view_trail {
        offset
    } else if lead < view_lead && trail > view_trail {
        // Target already covers the entire viewport — nearest is no move.
        offset
    } else if lead < view_lead {
        lead
    } else {
        trail - viewport
    };
    new_offset.clamp(0.0, max)
}

/// Core-owned, supported subset of inbound AccessKit actions (ADR-0098).
///
/// AccessKit's `Action` is a wide protocol vocabulary; Core maps it down to the
/// operations it actually drives and folds everything else to `Ignored`, so the
/// inbound surface is total and the runtime never sees a native-only concept.
/// The mapping lives entirely in Core (Rust API) and is never put on the proto
/// wire (ADR-0098 Decision 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessibilityAction {
    /// Move focus to `target`, driving the existing focus state machine.
    Focus { target: ElementId },
    /// Activate `target` by emitting a `Click` event directly to it — the
    /// semantic equivalent of a tap (ADR-0098 Decision 1), not a synthetic
    /// pointer replay. Skips hit-testing, `:active`, multi-click counting and
    /// the focus gesture.
    Click { target: ElementId },
    /// Replace `target`'s text-input value with `value` (ADR-0098 Decision 4).
    /// Any active preedit is finalized before the replacement so composition
    /// never lingers across it (same integrity as `element_paste`). A no-op for
    /// non-`text-input` targets.
    SetValue { target: ElementId, value: String },
    /// Bring `target` into view by adjusting the Scroll Offset of its nearest
    /// ancestor `scroll-view` (ADR-0098 Decision 4). Core sets the basic offset
    /// only — inertia/snap/rubber-band physics stay with the Platform Adapter
    /// and an AT-driven scroll carries none. A no-op when `target` has no
    /// scroll-view ancestor or is already fully visible.
    ScrollIntoView { target: ElementId },
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
        // AccessKit folds "default activation" into `Action::Click`; there is no
        // separate `Default` variant, so the ADR's "Click/Default" maps here.
        Action::Click => AccessibilityAction::Click { target },
        // `SetValue` carries the new text in `data`. Only a string `Value`
        // payload addresses a text-input; a missing or non-string payload (e.g.
        // a numeric slider value) has nothing to set, so it folds to `Ignored`.
        Action::SetValue => match &req.data {
            Some(ActionData::Value(value)) => AccessibilityAction::SetValue {
                target,
                value: value.to_string(),
            },
            _ => AccessibilityAction::Ignored,
        },
        Action::ScrollIntoView => AccessibilityAction::ScrollIntoView { target },
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
            AccessibilityAction::Click { target } => self.emit_semantic_click(target),
            AccessibilityAction::SetValue { target, value } => self.apply_set_value(target, &value),
            AccessibilityAction::ScrollIntoView { target } => self.scroll_into_view(target),
            AccessibilityAction::Ignored => {}
        }
    }

    /// Bring `target` into view by setting the Scroll Offset of its nearest
    /// ancestor `scroll-view` (ADR-0098 Decision 4). Core computes the minimal
    /// offset that makes the target's bounds visible — leading edge when the
    /// target is before the viewport, trailing edge when it is past — and leaves
    /// the offset untouched when the target is already fully visible. Emits a
    /// `Scroll` delivery on the scroll-view when (and only when) the offset
    /// actually moves. No inertia or snap physics: an AT-driven scroll sets the
    /// basic offset only. A no-op when `target` has no scroll-view ancestor.
    fn scroll_into_view(&mut self, target: ElementId) {
        let Some(scroll_view) = super::tree::next_ancestor_scroll_view(self, target) else {
            return;
        };
        let (Some((sx, sy, sw, sh)), Some((tx, ty, tw, th))) = (
            self.element_layout_rect(scroll_view),
            self.element_layout_rect(target),
        ) else {
            return;
        };

        let (ox, oy) = self.element_get_scroll_offset(scroll_view);
        let (content_w, content_h) = self.element_content_size(scroll_view);
        let max_x = (content_w - sw).max(0.0);
        let max_y = (content_h - sh).max(0.0);

        // layout_cache holds unscrolled content-space positions; the offset is
        // applied as a downstream transform (ADR-0022), so the target's position
        // within the content is `(tx - sx, ty - sy)`, independent of the offset.
        let new_x = scroll_axis_to_reveal(tx - sx, tw, sw, ox, max_x);
        let new_y = scroll_axis_to_reveal(ty - sy, th, sh, oy, max_y);

        if (new_x - ox).abs() < 1e-3 && (new_y - oy).abs() < 1e-3 {
            return;
        }
        self.element_set_scroll_offset(scroll_view, new_x, new_y);
        self.dispatch_event(
            DocumentEventKind::Scroll,
            Event::Scroll {
                target_id: scroll_view,
                delta_x: new_x - ox,
                delta_y: new_y - oy,
            },
        );
    }

    /// Emit a `Click` straight to `target` as a semantic activation (ADR-0098
    /// Decision 1). The coordinate is the target's layout centre so existing
    /// coordinate-reading listeners stay compatible without a wire change; the
    /// event then bubbles and dispatches like any other `Click`. Bypasses the
    /// pointer pipeline entirely — no hit-test, no `:active`, no multi-click
    /// counter, no focus gesture.
    fn emit_semantic_click(&mut self, target: ElementId) {
        let (x, y) = self
            .layout
            .layout_cache
            .get(&target)
            .map(|&(x, y, w, h)| (x + w / 2.0, y + h / 2.0))
            .unwrap_or((0.0, 0.0));
        self.dispatch_event(
            DocumentEventKind::Click,
            Event::Click {
                target_id: target,
                x,
                y,
            },
        );
    }

    /// Apply an AccessKit `SetValue` to `target` as a semantic value replacement
    /// (ADR-0098 Decision 4): finalize any active preedit, replace the
    /// text-input's content with `value`, then queue a `TextInput` event so app
    /// listeners observe it like any other edit. A no-op for non-`text-input`
    /// targets, and silent when the displayed value is unchanged.
    fn apply_set_value(&mut self, target: ElementId, value: &str) {
        let el = match self.elements.get_mut(&target) {
            Some(e) if e.kind == ElementKind::TextInput => e,
            _ => return,
        };
        let Some(edit) = el.edit.as_mut() else {
            return;
        };
        if !edit.set_value(value) {
            return;
        }
        self.dispatch_event(
            DocumentEventKind::TextInput,
            Event::TextInput {
                target_id: target,
                text: value.to_string(),
            },
        );
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
    use crate::element::{
        Dimension, DisplayValue, DocumentEventKind, Event, PositionValue, StyleProp,
    };
    use accesskit::{Action, ActionData, ActionRequest};

    /// A vertical scroll-view (200×100 viewport) whose 500px-tall content holds a
    /// 50px `target` pinned at content-y 300 — far below the viewport. Used to
    /// exercise `ScrollIntoView`. Returns `(tree, scroll, target)`.
    fn scroll_into_view_scene() -> (ElementTree, ElementId, ElementId) {
        let mut tree = ElementTree::new();
        let scroll = tree.element_create(1, ElementKind::ScrollView);
        let content = tree.element_create(2, ElementKind::View);
        let target = tree.element_create(3, ElementKind::View);
        tree.set_root(scroll);
        tree.set_viewport(400.0, 400.0);
        tree.element_set_style(
            scroll,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            content,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(500.0)),
            ],
        );
        tree.element_append_child(scroll, content);
        tree.element_set_style(
            target,
            &[
                StyleProp::Position(PositionValue::Absolute),
                StyleProp::Top(Dimension::px(300.0)),
                StyleProp::Left(Dimension::px(0.0)),
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(50.0)),
            ],
        );
        tree.element_append_child(content, target);
        tree.render(0.0);
        (tree, scroll, target)
    }

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
    fn maps_click_action_to_core_click_resolving_element_id() {
        let target = ElementId::from_u64(9);
        let mapped = map_action_request(&action_request(Action::Click, node_id(target)));
        assert_eq!(mapped, AccessibilityAction::Click { target });
    }

    #[test]
    fn maps_set_value_action_with_value_payload() {
        let target = ElementId::from_u64(11);
        let req = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node_id(target),
            data: Some(ActionData::Value("hello".into())),
        };
        assert_eq!(
            map_action_request(&req),
            AccessibilityAction::SetValue {
                target,
                value: "hello".to_string(),
            }
        );
    }

    #[test]
    fn maps_set_value_without_value_payload_to_ignored() {
        let node = node_id(ElementId::from_u64(5));
        let req = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: None,
        };
        assert_eq!(
            map_action_request(&req),
            AccessibilityAction::Ignored,
            "SetValue with no string value has nothing to set",
        );
    }

    #[test]
    fn maps_scroll_into_view_action_to_core_scroll_into_view_resolving_element_id() {
        let target = ElementId::from_u64(13);
        let mapped = map_action_request(&action_request(Action::ScrollIntoView, node_id(target)));
        assert_eq!(mapped, AccessibilityAction::ScrollIntoView { target });
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

    fn set_value_request(node: NodeId, value: &str) -> ActionRequest {
        ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: Some(ActionData::Value(value.into())),
        }
    }

    #[test]
    fn on_accessibility_action_set_value_replaces_content_and_delivers_text_input() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let input = tree.element_create(2, ElementKind::TextInput);
        tree.element_append_child(root, input);
        tree.element_set_text_content(input, "old");

        let listener = tree.register_listener(input, DocumentEventKind::TextInput);
        tree.on_accessibility_action(set_value_request(node_id(input), "new value"));

        assert_eq!(
            tree.element_get_text_content(input),
            "new value",
            "SetValue must replace the text-input's content",
        );
        let deliveries = tree.poll_deliveries();
        let ids: Vec<_> = deliveries.iter().map(|d| d.listener_id).collect();
        assert_eq!(
            ids,
            vec![listener],
            "the replacement fires a TextInput delivery"
        );
        assert!(
            matches!(
                &deliveries[0].event,
                Event::TextInput { text, target_id } if text == "new value" && *target_id == input
            ),
            "delivered event carries the new value and targets the input",
        );
    }

    #[test]
    fn on_accessibility_action_set_value_finalizes_active_preedit_then_replaces() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let input = tree.element_create(2, ElementKind::TextInput);
        tree.element_append_child(root, input);
        tree.element_set_text_content(input, "abc");
        tree.element_set_preedit(input, "DEF"); // in-progress IME composition

        tree.on_accessibility_action(set_value_request(node_id(input), "xyz"));

        // The preedit is finalized as part of the replacement — no broken
        // intermediate state (prior art: `element_paste` preedit confirmation).
        assert_eq!(tree.element_get_text_content(input), "xyz");
        // Clearing the (already-finalized) preedit afterward changes nothing,
        // proving composition did not linger across the replacement.
        tree.element_set_preedit(input, "");
        assert_eq!(tree.element_get_text_content(input), "xyz");
    }

    #[test]
    fn on_accessibility_action_set_value_on_non_text_input_is_noop() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let view = tree.element_create(2, ElementKind::View);
        tree.element_append_child(root, view);
        tree.register_listener(view, DocumentEventKind::TextInput);

        tree.on_accessibility_action(set_value_request(node_id(view), "nope"));
        assert!(
            tree.poll_deliveries().is_empty(),
            "SetValue on a non-text-input target must emit nothing",
        );
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
    fn on_accessibility_action_click_emits_bubbling_click_to_listeners() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.render(0.0);

        let l_btn = tree.register_listener(button, DocumentEventKind::Click);
        let l_root = tree.register_listener(root, DocumentEventKind::Click);

        tree.on_accessibility_action(action_request(Action::Click, node_id(button)));

        let deliveries = tree.poll_deliveries();
        let ids: Vec<_> = deliveries.iter().map(|d| d.listener_id).collect();
        assert_eq!(ids, vec![l_btn, l_root], "Click must bubble target → root");
        assert!(
            matches!(deliveries[0].event, Event::Click { target_id, .. } if target_id == button),
            "delivered event must be a Click targeting the requested node"
        );
    }

    #[test]
    fn on_accessibility_action_click_uses_target_layout_center() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.element_set_style(
            button,
            &[
                StyleProp::Width(Dimension::px(120.0)),
                StyleProp::Height(Dimension::px(40.0)),
            ],
        );
        tree.render(0.0);

        let (rx, ry, rw, rh) = tree.element_layout_rect(button).expect("button layout");
        let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);

        tree.register_listener(button, DocumentEventKind::Click);
        tree.on_accessibility_action(action_request(Action::Click, node_id(button)));

        let delivery = tree.poll_deliveries().pop().expect("a click delivery");
        match delivery.event {
            Event::Click { x, y, .. } => {
                assert_eq!((x, y), (cx, cy), "click must land at the target's layout center");
            }
            other => panic!("expected a Click event, got {other:?}"),
        }
    }

    #[test]
    fn on_accessibility_action_click_does_not_flush_active_state() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.render(0.0);

        let l_active_start = tree.register_listener(button, DocumentEventKind::ActiveStart);
        let l_active_end = tree.register_listener(button, DocumentEventKind::ActiveEnd);

        tree.on_accessibility_action(action_request(Action::Click, node_id(button)));

        let fired: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert!(
            !fired.contains(&l_active_start) && !fired.contains(&l_active_end),
            "semantic click must not fire :active (ActiveStart/ActiveEnd)"
        );
        assert_eq!(
            tree.active_element(),
            None,
            "semantic click must leave no element in the :active state"
        );
    }

    #[test]
    fn on_accessibility_action_click_does_not_hit_test() {
        use crate::element::PositionValue;

        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);

        // Target lays out at the top-left, 100x100 → centre (50, 50).
        let target = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, target);
        tree.element_set_style(
            target,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );

        // An absolutely-positioned overlay sits on top of the target's centre,
        // so a coordinate hit-test at (50, 50) would resolve to the overlay.
        let overlay = tree.element_create(3, ElementKind::View);
        tree.element_append_child(root, overlay);
        tree.element_set_style(
            overlay,
            &[
                StyleProp::Position(PositionValue::Absolute),
                StyleProp::Top(Dimension::px(0.0)),
                StyleProp::Left(Dimension::px(0.0)),
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.render(0.0);

        // Precondition: a hit-test at the target's centre would pick the overlay,
        // so delivering to the target proves the AT path never hit-tested.
        assert_eq!(
            tree.hit_test(50.0, 50.0),
            Some(overlay),
            "test setup: overlay must cover the target's centre",
        );

        let l_target = tree.register_listener(target, DocumentEventKind::Click);
        let l_overlay = tree.register_listener(overlay, DocumentEventKind::Click);

        tree.on_accessibility_action(action_request(Action::Click, node_id(target)));

        let ids: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert!(
            ids.contains(&l_target),
            "the AT-targeted element must receive the click"
        );
        assert!(
            !ids.contains(&l_overlay),
            "the overlay over the centre must not receive it — no hit-test runs"
        );
    }

    #[test]
    fn on_accessibility_action_click_does_not_advance_multi_click_counter() {
        fn paragraph() -> (ElementTree, ElementId) {
            let mut tree = ElementTree::new();
            let view = tree.element_create(1, ElementKind::View);
            let text = tree.element_create(2, ElementKind::Text);
            tree.set_root(view);
            tree.set_viewport(400.0, 200.0);
            tree.element_set_style(
                view,
                &[
                    StyleProp::Width(Dimension::px(400.0)),
                    StyleProp::Height(Dimension::px(200.0)),
                ],
            );
            tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
            tree.element_append_child(view, text);
            tree.element_set_text(text, "Hello world");
            tree.element_set_selectable(view, true);
            tree.render(0.0);
            (tree, text)
        }
        let (px, py) = (10.0, 8.0);
        fn range(tree: &ElementTree, text: ElementId) -> Option<(usize, usize)> {
            tree.selection().and_then(|s| s.range_within(text))
        }

        // The pointer multi-click counter cycles: two same-spot presses select a
        // word, three select the whole line — establishing the phases differ.
        let (mut t2, text2) = paragraph();
        t2.on_pointer_down(px, py);
        t2.on_pointer_down(px, py);
        let word = range(&t2, text2);

        let (mut t3, text3) = paragraph();
        t3.on_pointer_down(px, py);
        t3.on_pointer_down(px, py);
        t3.on_pointer_down(px, py);
        let line = range(&t3, text3);

        assert!(word.is_some() && line.is_some(), "presses must select");
        assert_ne!(word, line, "the counter must cycle word → line");

        // A semantic click between two real presses must not advance the counter,
        // so the second press still lands on the word phase, not the line phase.
        let (mut t, text) = paragraph();
        t.on_pointer_down(px, py);
        t.on_accessibility_action(action_request(Action::Click, node_id(text)));
        t.on_pointer_down(px, py);
        assert_eq!(
            range(&t, text),
            word,
            "semantic click must not advance the multi-click counter",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_reveals_offscreen_target() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        assert_eq!(
            tree.element_get_scroll_offset(scroll),
            (0.0, 0.0),
            "precondition: scroll-view starts unscrolled",
        );

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        // The target sits at content-y 300..350 in a 100px viewport, so the
        // minimal scroll aligns its bottom to the viewport bottom: offset 250.
        let (_, oy) = tree.element_get_scroll_offset(scroll);
        assert!(
            (oy - 250.0).abs() < 0.5,
            "scroll offset must reveal the target, got {oy}",
        );

        // After scrolling, the target lies fully inside the viewport.
        let (_, ty, _, th) = tree.element_layout_rect(target).expect("target layout");
        let (_, sy, _, sh) = tree.element_layout_rect(scroll).expect("scroll layout");
        let rel_top = (ty - sy) - oy;
        assert!(
            rel_top >= -0.5 && rel_top + th <= sh + 0.5,
            "target must be fully visible: rel_top={rel_top}, th={th}, sh={sh}",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_scrolls_up_to_reveal_target_above_viewport() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        // Scroll to the bottom (max offset 400): the viewport now shows content
        // 400..500, leaving the target (300..350) above and out of view.
        tree.element_set_scroll_offset(scroll, 0.0, 400.0);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        // Minimal scroll up aligns the target's leading (top) edge to the
        // viewport top: offset 300.
        let (_, oy) = tree.element_get_scroll_offset(scroll);
        assert!(
            (oy - 300.0).abs() < 0.5,
            "scrolling up must align the target's top edge, got {oy}",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_emits_scroll_delivery_on_scroll_view() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        let listener = tree.register_listener(scroll, DocumentEventKind::Scroll);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        let deliveries = tree.poll_deliveries();
        let ids: Vec<_> = deliveries.iter().map(|d| d.listener_id).collect();
        assert_eq!(
            ids,
            vec![listener],
            "the offset change must fire a Scroll delivery on the scroll-view",
        );
        assert!(
            matches!(
                deliveries[0].event,
                Event::Scroll { target_id, .. } if target_id == scroll
            ),
            "delivered event must be a Scroll targeting the scroll-view",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_leaves_visible_target_untouched() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        // Pre-scroll so the target (content-y 300..350) already sits fully inside
        // the 100px viewport at offset 260 (rel 40..90).
        tree.element_set_scroll_offset(scroll, 0.0, 260.0);
        let listener = tree.register_listener(scroll, DocumentEventKind::Scroll);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        assert_eq!(
            tree.element_get_scroll_offset(scroll),
            (0.0, 260.0),
            "an already-visible target must not move the offset",
        );
        assert!(
            tree.poll_deliveries().is_empty(),
            "no offset change means no Scroll delivery",
        );
        let _ = listener;
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_without_scroll_view_ancestor_is_noop() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let target = tree.element_create(2, ElementKind::View);
        tree.element_append_child(root, target);
        tree.render(0.0);

        let l_root = tree.register_listener(root, DocumentEventKind::Scroll);
        let l_target = tree.register_listener(target, DocumentEventKind::Scroll);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        assert!(
            tree.poll_deliveries().is_empty(),
            "ScrollIntoView on a target with no scroll-view ancestor must do nothing",
        );
        let _ = (l_root, l_target);
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
