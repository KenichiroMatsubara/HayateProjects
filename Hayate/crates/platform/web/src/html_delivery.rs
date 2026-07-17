//! HTML Mode の配信経路 — DOM を使わないネイティブ統合テスト。
//!
//! `HayateElementHtmlRenderer` は入力を `ElementTree::on_*` へ流し、
//! `poll_events()` から `tree.poll_deliveries()` を排出する。本ハーネスは
//! ユニットテスト用にそのイベントルーティングを再現する。

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use hayate_core::{
        Dimension, DocumentEventKind, ElementId, ElementKind, ElementTree, Event, ListenerId,
        StyleProp,
    };

    /// `HayateElementHtmlRenderer` の `tree` とノード所属ゲートを再現する。
    struct HtmlDeliveryHarness {
        tree: ElementTree,
        nodes: HashSet<ElementId>,
    }

    impl HtmlDeliveryHarness {
        fn new() -> Self {
            Self {
                tree: ElementTree::new(),
                nodes: HashSet::new(),
            }
        }

        fn create(&mut self, id: u64, kind: ElementKind) -> ElementId {
            let eid = self.tree.element_create(id, kind);
            self.nodes.insert(eid);
            eid
        }

        fn set_root(&mut self, id: ElementId) {
            self.tree.set_root(id);
        }

        fn append_child(&mut self, parent: ElementId, child: ElementId) {
            self.tree.element_append_child(parent, child);
        }

        fn register_listener(
            &mut self,
            element_id: ElementId,
            kind: DocumentEventKind,
        ) -> ListenerId {
            self.tree.register_listener(element_id, kind)
        }

        fn on_pointer_down(&mut self, target_id: u64, x: f32, y: f32) {
            let target = ElementId::from_u64(target_id);
            if self.nodes.contains(&target) {
                self.tree.on_pointer_down_on(target, x, y);
            }
        }

        fn on_pointer_up(&mut self, target_id: u64) {
            let target = ElementId::from_u64(target_id);
            let fallback = self.nodes.contains(&target).then_some(target);
            self.tree.on_pointer_up_on(fallback);
        }

        fn on_wheel(&mut self, target_id: u64, delta_x: f32, delta_y: f32) {
            let target = ElementId::from_u64(target_id);
            if !self.nodes.contains(&target) {
                return;
            }
            if let Some(sv) = self.tree.apply_wheel_delta(target, delta_x, delta_y) {
                let _ = self.tree.element_get_scroll_offset(sv);
            }
            self.tree.on_wheel(target, delta_x, delta_y);
        }

        fn on_resize(&mut self, width: f32, height: f32) {
            self.tree.set_viewport(width, height);
            self.tree.on_resize(width, height);
        }

        fn poll_deliveries(&mut self) -> Vec<hayate_core::EventDelivery> {
            self.tree.poll_deliveries()
        }
    }

    fn scroll_tree(content_h: f32) -> HtmlDeliveryHarness {
        let mut h = HtmlDeliveryHarness::new();
        let sv = h.create(1, ElementKind::ScrollView);
        let child = h.create(2, ElementKind::View);
        h.set_root(sv);
        h.append_child(sv, child);
        h.tree.element_set_style(
            sv,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        h.tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(content_h)),
            ],
        );
        h.tree.render(0.0);
        h
    }

    #[test]
    fn wheel_updates_scroll_offset_and_delivers_scroll_listener() {
        let mut h = scroll_tree(300.0);
        let child = ElementId::from_u64(2);
        let sv = ElementId::from_u64(1);
        let listener = h.register_listener(child, DocumentEventKind::Scroll);

        h.on_wheel(2, 0.0, 25.0);

        let (_, y) = h.tree.element_get_scroll_offset(sv);
        assert!((y - 25.0).abs() < 1e-3);

        let deliveries = h.poll_deliveries();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].listener_id, listener);
        assert!(matches!(
            &deliveries[0].event,
            Event::Scroll {
                target_id,
                delta_x,
                delta_y
            } if *target_id == child && (*delta_x).abs() < f32::EPSILON && (*delta_y - 25.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn click_bubbles_through_element_tree() {
        let mut h = HtmlDeliveryHarness::new();
        let root = h.create(10, ElementKind::View);
        let leaf = h.create(11, ElementKind::Button);
        h.set_root(root);
        h.append_child(root, leaf);

        let l_root = h.register_listener(root, DocumentEventKind::Click);
        let l_leaf = h.register_listener(leaf, DocumentEventKind::Click);

        // クリックはリリースで確定する（ADR-0082）。押して離すと leaf でタップが
        // 解決し祖先 root まで bubble する。
        h.on_pointer_down(11, 4.0, 5.0);
        h.on_pointer_up(11);

        let ids: Vec<_> = h
            .poll_deliveries()
            .into_iter()
            .filter(|d| matches!(d.event, Event::Click { .. }))
            .map(|d| d.listener_id)
            .collect();
        assert_eq!(ids, vec![l_leaf, l_root]);
    }

    #[test]
    fn resize_is_host_echo_and_produces_no_delivery() {
        let mut h = HtmlDeliveryHarness::new();
        let root = h.create(1, ElementKind::View);
        h.set_root(root);
        let _listener = h.register_listener(root, DocumentEventKind::Click);

        h.on_resize(640.0, 480.0);

        assert!(h.poll_deliveries().is_empty());
    }

    #[test]
    fn ignores_events_for_unknown_node_ids() {
        let mut h = HtmlDeliveryHarness::new();
        let btn = h.create(1, ElementKind::Button);
        h.set_root(btn);
        let listener = h.register_listener(btn, DocumentEventKind::Click);

        h.on_pointer_down(99, 0.0, 0.0);

        assert!(h.poll_deliveries().is_empty());
        let _ = listener;
    }

    #[test]
    fn delivery_queue_drains_on_poll() {
        let mut h = HtmlDeliveryHarness::new();
        let btn = h.create(1, ElementKind::Button);
        h.set_root(btn);
        h.register_listener(btn, DocumentEventKind::Click);

        // クリックはリリースで確定する（ADR-0082）ので、押して離して 1 件を配信する。
        h.on_pointer_down(1, 0.0, 0.0);
        h.on_pointer_up(1);
        assert_eq!(h.poll_deliveries().len(), 1);
        assert!(h.poll_deliveries().is_empty());
    }
}
