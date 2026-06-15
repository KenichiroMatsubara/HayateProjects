use std::collections::HashMap;

use slotmap::{DefaultKey, KeyData, SlotMap};

use crate::element::event_spec::{DocumentEventKind, Event};
use crate::element::id::ElementId;

/// Opaque listener handle issued by `register_listener`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenerId(DefaultKey);

impl ListenerId {
    pub fn to_u64(self) -> u64 {
        slotmap::Key::data(&self.0).as_ffi()
    }

    pub fn from_u64(raw: u64) -> Self {
        Self(DefaultKey::from(KeyData::from_ffi(raw)))
    }
}

/// A single delivery queued for host `poll_events` drain (ADR-0053).
#[derive(Clone, Debug)]
pub struct EventDelivery {
    pub listener_id: ListenerId,
    pub event: Event,
}

struct ListenerEntry {
    element_id: ElementId,
    kind: DocumentEventKind,
}

/// Element Document Runtime: listener registry, bubble dispatch, delivery queue.
pub struct DocumentRuntime {
    listeners: SlotMap<DefaultKey, ListenerEntry>,
    by_element: HashMap<ElementId, HashMap<DocumentEventKind, Vec<DefaultKey>>>,
    delivery_queue: Vec<EventDelivery>,
}

impl DocumentRuntime {
    pub fn new() -> Self {
        Self {
            listeners: SlotMap::with_key(),
            by_element: HashMap::new(),
            delivery_queue: Vec::new(),
        }
    }

    pub fn register_listener(
        &mut self,
        element_id: ElementId,
        kind: DocumentEventKind,
    ) -> ListenerId {
        let key = self.listeners.insert(ListenerEntry { element_id, kind });
        self.by_element
            .entry(element_id)
            .or_default()
            .entry(kind)
            .or_default()
            .push(key);
        ListenerId(key)
    }

    pub fn unregister_listener(&mut self, id: ListenerId) -> bool {
        let entry = match self.listeners.remove(id.0) {
            Some(e) => e,
            None => return false,
        };
        if let Some(kinds) = self.by_element.get_mut(&entry.element_id) {
            if let Some(list) = kinds.get_mut(&entry.kind) {
                list.retain(|k| *k != id.0);
            }
        }
        true
    }

    pub fn remove_element_listeners(&mut self, element_id: ElementId) {
        if let Some(kinds) = self.by_element.remove(&element_id) {
            for keys in kinds.into_values() {
                for key in keys {
                    self.listeners.remove(key);
                }
            }
        }
    }

    /// Dispatch `event` to listeners on `path` (target-first ancestor chain).
    pub fn dispatch_to_path(
        &mut self,
        path: &[ElementId],
        kind: DocumentEventKind,
        event: Event,
    ) {
        for &element_id in path {
            if let Some(listeners) = self
                .by_element
                .get(&element_id)
                .and_then(|kinds| kinds.get(&kind))
            {
                for &key in listeners {
                    self.delivery_queue.push(EventDelivery {
                        listener_id: ListenerId(key),
                        event: event.clone(),
                    });
                }
            }
        }
    }

    pub fn poll_deliveries(&mut self) -> Vec<EventDelivery> {
        std::mem::take(&mut self.delivery_queue)
    }
}

impl Default for DocumentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn event_target(event: &Event) -> Option<ElementId> {
    match event {
        Event::Click { target_id, .. }
        | Event::Focus { target_id }
        | Event::Blur { target_id }
        | Event::TextInput { target_id, .. }
        | Event::CompositionStart { target_id, .. }
        | Event::CompositionUpdate { target_id, .. }
        | Event::CompositionEnd { target_id, .. }
        | Event::Scroll { target_id, .. }
        | Event::HoverEnter { target_id }
        | Event::HoverLeave { target_id }
        | Event::ActiveStart { target_id }
        | Event::ActiveEnd { target_id }
        | Event::KeyDown { target_id, .. } => Some(*target_id),
        Event::Resize { .. }
        | Event::PointerMove { .. }
        | Event::FetchFont { .. }
        | Event::SelectionChange => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::kind::ElementKind;
    use crate::element::style::{Dimension, StyleProp};
    use crate::element::tree::ElementTree;

    fn scroll_tree(content_h: f32) -> (ElementTree, ElementId, ElementId) {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::ScrollView);
        let child = tree.element_create(2, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(200.0, 100.0);
        tree.element_append_child(root, child);
        tree.element_set_style(
            root,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(content_h)),
            ],
        );
        tree.render(0.0);
        (tree, root, child)
    }

    #[test]
    fn bubble_dispatches_to_ancestors_in_order() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(10, ElementKind::View);
        let mid = tree.element_create(11, ElementKind::View);
        let leaf = tree.element_create(12, ElementKind::Button);
        tree.set_root(root);
        tree.element_append_child(root, mid);
        tree.element_append_child(mid, leaf);

        let l_root = tree.register_listener(root, DocumentEventKind::Click);
        let l_mid = tree.register_listener(mid, DocumentEventKind::Click);
        let l_leaf = tree.register_listener(leaf, DocumentEventKind::Click);

        let event = Event::Click {
            target_id: leaf,
            x: 1.0,
            y: 2.0,
        };
        tree.dispatch_event(DocumentEventKind::Click, event);

        let deliveries = tree.poll_deliveries();
        assert_eq!(deliveries.len(), 3);
        assert_eq!(deliveries[0].listener_id, l_leaf);
        assert_eq!(deliveries[1].listener_id, l_mid);
        assert_eq!(deliveries[2].listener_id, l_root);
    }

    #[test]
    fn non_bubble_stops_at_target() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(20, ElementKind::View);
        let leaf = tree.element_create(21, ElementKind::TextInput);
        tree.set_root(root);
        tree.element_append_child(root, leaf);

        let l_root = tree.register_listener(root, DocumentEventKind::Focus);
        let l_leaf = tree.register_listener(leaf, DocumentEventKind::Focus);

        tree.dispatch_event(
            DocumentEventKind::Focus,
            Event::Focus {
                target_id: leaf,
            },
        );

        let deliveries = tree.poll_deliveries();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].listener_id, l_leaf);
        assert_ne!(deliveries[0].listener_id, l_root);
    }

    #[test]
    fn multiple_listeners_on_same_element() {
        let mut tree = ElementTree::new();
        let btn = tree.element_create(30, ElementKind::Button);
        tree.set_root(btn);

        let l1 = tree.register_listener(btn, DocumentEventKind::Click);
        let l2 = tree.register_listener(btn, DocumentEventKind::Click);

        tree.dispatch_event(
            DocumentEventKind::Click,
            Event::Click {
                target_id: btn,
                x: 0.0,
                y: 0.0,
            },
        );

        let ids: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert_eq!(ids, vec![l1, l2]);
    }

    #[test]
    fn apply_wheel_delta_clamps_at_bounds() {
        let (mut tree, sv, child) = scroll_tree(300.0);
        tree.element_set_scroll_offset(sv, 0.0, 180.0);

        tree.apply_wheel_delta(child, 0.0, 50.0);
        let (_, y) = tree.element_get_scroll_offset(sv);
        assert!((y - 200.0).abs() < 1e-3, "expected clamp at max_y=200, got {y}");

        tree.apply_wheel_delta(child, 0.0, -500.0);
        let (_, y) = tree.element_get_scroll_offset(sv);
        assert!(y.abs() < 1e-3, "expected clamp at 0, got {y}");
    }

    #[test]
    fn apply_wheel_delta_finds_nearest_scroll_view() {
        let (mut tree, sv, child) = scroll_tree(300.0);
        tree.apply_wheel_delta(child, 0.0, 10.0);
        let (_, y) = tree.element_get_scroll_offset(sv);
        assert!((y - 10.0).abs() < 1e-3);
    }

    fn nested_scroll_tree() -> (ElementTree, ElementId, ElementId, ElementId) {
        let mut tree = ElementTree::new();
        let outer = tree.element_create(100, ElementKind::ScrollView);
        let inner = tree.element_create(101, ElementKind::ScrollView);
        let leaf = tree.element_create(102, ElementKind::View);
        let tail = tree.element_create(103, ElementKind::View);
        tree.set_root(outer);
        tree.set_viewport(240.0, 240.0);
        tree.element_append_child(outer, inner);
        tree.element_append_child(inner, leaf);
        tree.element_append_child(outer, tail);
        tree.element_set_style(
            outer,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(200.0)),
            ],
        );
        tree.element_set_style(
            inner,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            leaf,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(300.0)),
            ],
        );
        tree.element_set_style(
            tail,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(250.0)),
            ],
        );
        tree.render(0.0);
        (tree, outer, inner, leaf)
    }

    #[test]
    fn apply_wheel_delta_chains_to_ancestor_when_inner_at_edge() {
        let (mut tree, outer, inner, leaf) = nested_scroll_tree();
        tree.element_set_scroll_offset(inner, 0.0, 200.0);

        tree.apply_wheel_delta(leaf, 0.0, 40.0);

        let (_, inner_y) = tree.element_get_scroll_offset(inner);
        assert!(
            (inner_y - 200.0).abs() < 1e-3,
            "inner should stay clamped at max, got {inner_y}"
        );
        let (_, outer_y) = tree.element_get_scroll_offset(outer);
        assert!(
            (outer_y - 40.0).abs() < 1e-3,
            "outer should absorb chained delta, got {outer_y}"
        );
    }

    fn nested_scroll_tree_axis_split() -> (ElementTree, ElementId, ElementId, ElementId) {
        let mut tree = ElementTree::new();
        let outer = tree.element_create(200, ElementKind::ScrollView);
        let inner = tree.element_create(201, ElementKind::ScrollView);
        let leaf = tree.element_create(202, ElementKind::View);
        let tail = tree.element_create(203, ElementKind::View);
        tree.set_root(outer);
        tree.set_viewport(240.0, 240.0);
        tree.element_append_child(outer, inner);
        tree.element_append_child(inner, leaf);
        tree.element_append_child(outer, tail);
        tree.element_set_style(
            outer,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(200.0)),
            ],
        );
        tree.element_set_style(
            inner,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::MaxWidth(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            leaf,
            &[
                StyleProp::MinWidth(Dimension::px(400.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            tail,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(250.0)),
            ],
        );
        tree.render(0.0);
        (tree, outer, inner, leaf)
    }

    #[test]
    fn apply_wheel_delta_chains_axes_independently() {
        let (mut tree, outer, inner, leaf) = nested_scroll_tree_axis_split();
        let (inner_cw, inner_ch) = tree.element_content_size(inner);
        let inner_rect = tree.element_layout_rect(inner).unwrap();
        let inner_max_x = (inner_cw - inner_rect.2).max(0.0);
        let inner_max_y = (inner_ch - inner_rect.3).max(0.0);
        assert!(
            inner_max_x > 50.0,
            "inner must scroll horizontally (max_x={inner_max_x})"
        );
        assert!(
            inner_max_y < 1e-3,
            "inner must not scroll vertically (max_y={inner_max_y})"
        );
        let (_, outer_ch) = tree.element_content_size(outer);
        let outer_rect = tree.element_layout_rect(outer).unwrap();
        let outer_max_y = (outer_ch - outer_rect.3).max(0.0);
        assert!(
            outer_max_y > 30.0,
            "outer must scroll vertically (max_y={outer_max_y})"
        );

        tree.apply_wheel_delta(leaf, 50.0, 30.0);

        let (inner_x, inner_y) = tree.element_get_scroll_offset(inner);
        assert!(
            (inner_x - 50.0).abs() < 1e-3,
            "inner should consume horizontal delta, got {inner_x}"
        );
        assert!(
            inner_y.abs() < 1e-3,
            "inner should not scroll vertically, got {inner_y}"
        );
        let (outer_x, outer_y) = tree.element_get_scroll_offset(outer);
        assert!(
            outer_x.abs() < 1e-3,
            "outer should not scroll horizontally when inner consumed x, got {outer_x}"
        );
        assert!(
            (outer_y - 30.0).abs() < 1e-3,
            "outer should consume chained vertical delta, got {outer_y}"
        );
    }

    #[test]
    fn apply_wheel_delta_drops_remainder_without_scroll_view_ancestor() {
        let (mut tree, sv, child) = scroll_tree(300.0);
        tree.element_set_scroll_offset(sv, 0.0, 200.0);

        tree.apply_wheel_delta(child, 0.0, 40.0);

        let (_, y) = tree.element_get_scroll_offset(sv);
        assert!(
            (y - 200.0).abs() < 1e-3,
            "single scroll view should stay clamped at max, got {y}"
        );
    }
}
