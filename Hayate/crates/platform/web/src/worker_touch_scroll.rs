use hayate_core::scroll::{
    estimate_release_velocity, is_drag_scroll_pointer, MoveOutcome, ScrollGesture,
    ScrollPhysicsTuning,
};
use hayate_core::{ElementTree, InteractionIntent, PointerKind, PointerRouting};

/// Worker-side adapter from browser pointer facts to Core's shared touch-scroll gesture.
///
/// The module owns only one in-flight gesture. Frame admission, coalescing, dirty propagation, and
/// GPU completion stay in `LatestWinsFramePipeline`.
pub(crate) struct WorkerTouchScroll {
    gesture: Option<ScrollGesture>,
    pending_tap: Option<(f32, f32, PointerKind)>,
    /// Worker receipt-time pointer facts consumed by Core's shared release-velocity estimator.
    samples: Vec<(f32, f32, f64)>,
    /// Same authoritative Scroll Physics Profile tuning installed on the Core tree.
    tuning: ScrollPhysicsTuning,
}

impl WorkerTouchScroll {
    pub(crate) fn new() -> Self {
        Self {
            gesture: None,
            pending_tap: None,
            samples: Vec::new(),
            tuning: ScrollPhysicsTuning::default(),
        }
    }

    pub(crate) fn set_tuning(&mut self, tuning: ScrollPhysicsTuning) {
        self.tuning = tuning;
    }

    pub(crate) fn pointer_down(
        &mut self,
        tree: &mut ElementTree,
        x: f32,
        y: f32,
        kind: PointerKind,
        _timestamp_ms: f64,
    ) {
        self.gesture = None;
        self.pending_tap = None;
        self.samples.clear();
        if is_drag_scroll_pointer(kind) {
            if let Some(scroll_view) = tree
                .hit_test(x, y)
                .and_then(|hit| tree.nearest_scroll_view(hit))
            {
                tree.prepare_deferred_pointer_down(kind);
                self.gesture = Some(ScrollGesture::new(scroll_view, (x, y)));
                self.pending_tap = Some((x, y, kind));
                return;
            }
        }
        let _ = tree.apply_interaction_intent(InteractionIntent::PointerDown {
            x,
            y,
            modifiers: 0,
            pointer_kind: kind,
            routing: PointerRouting::CanvasHitTest,
        });
    }

    pub(crate) fn pointer_move(
        &mut self,
        tree: &mut ElementTree,
        x: f32,
        y: f32,
        kind: PointerKind,
        timestamp_ms: f64,
    ) {
        let Some(mut gesture) = self.gesture.take() else {
            let _ = tree.apply_interaction_intent(InteractionIntent::PointerMove {
                x,
                y,
                pointer_kind: kind,
                routing: PointerRouting::CanvasHitTest,
            });
            return;
        };
        match gesture.on_move((x, y), self.tuning.slop_px) {
            MoveOutcome::Pending => {}
            MoveOutcome::StartScroll => {
                self.pending_tap = None;
                self.samples.push((x, y, timestamp_ms));
            }
            MoveOutcome::Scroll { dx, dy } => {
                let scroll_view = gesture.scroll_view;
                let (old_x, old_y) = tree.element_get_scroll_offset(scroll_view);
                let (max_x, max_y) = tree.element_scroll_max_offset(scroll_view);
                let next_x = (old_x + dx).clamp(0.0, max_x.max(0.0));
                let next_y = (old_y + dy).clamp(0.0, max_y.max(0.0));
                tree.element_set_scroll_offset(scroll_view, next_x, next_y);
                tree.on_wheel(scroll_view, next_x - old_x, next_y - old_y);
                self.samples.push((x, y, timestamp_ms));
            }
        }
        self.gesture = Some(gesture);
    }

    pub(crate) fn pointer_up(
        &mut self,
        tree: &mut ElementTree,
        x: f32,
        y: f32,
        kind: PointerKind,
        timestamp_ms: f64,
    ) {
        let gesture = self.gesture.take();
        let tap = gesture.is_none_or(|gesture| gesture.is_tap());
        if tap {
            if let Some((down_x, down_y, down_kind)) = self.pending_tap.take() {
                let _ = tree.apply_interaction_intent(InteractionIntent::PointerDown {
                    x: down_x,
                    y: down_y,
                    modifiers: 0,
                    pointer_kind: down_kind,
                    routing: PointerRouting::CanvasHitTest,
                });
            }
            let _ = tree.apply_interaction_intent(InteractionIntent::PointerUp {
                x,
                y,
                pointer_kind: kind,
                routing: PointerRouting::CanvasHitTest,
            });
        } else {
            self.pending_tap = None;
            self.samples.push((x, y, timestamp_ms));
            let (vx, vy) = estimate_release_velocity(&self.samples, &self.tuning);
            self.samples.clear();
            tree.start_scroll_momentum(
                gesture
                    .expect("non-tap release keeps its scroll gesture")
                    .scroll_view,
                vx,
                vy,
            );
        }
    }
}

impl Default for WorkerTouchScroll {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use hayate_core::element::style::{Dimension, StyleProp};
    use hayate_core::{ElementId, ElementKind, ElementTree, PointerKind};

    use super::WorkerTouchScroll;

    fn scrollable() -> (ElementTree, ElementId) {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        let scroll = tree.element_create(2, ElementKind::ScrollView);
        let content = tree.element_create(3, ElementKind::View);
        tree.set_root(root);
        tree.element_append_child(root, scroll);
        tree.element_append_child(scroll, content);
        tree.set_viewport(100.0, 100.0);
        tree.element_set_style(
            root,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            scroll,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            content,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(500.0)),
            ],
        );
        let _ = tree.commit_rendered_frame(0.0);
        (tree, scroll)
    }

    #[test]
    fn touch_drag_moves_the_hit_scroll_view_to_the_latest_pointer_position() {
        let (mut tree, scroll) = scrollable();
        let mut touch = WorkerTouchScroll::new();

        touch.pointer_down(&mut tree, 50.0, 80.0, PointerKind::Touch, 0.0);
        touch.pointer_move(&mut tree, 50.0, 60.0, PointerKind::Touch, 10.0);
        touch.pointer_move(&mut tree, 50.0, 20.0, PointerKind::Touch, 20.0);
        touch.pointer_up(&mut tree, 50.0, 20.0, PointerKind::Touch, 30.0);

        assert_eq!(tree.element_get_scroll_offset(scroll), (0.0, 40.0));
    }

    #[test]
    fn released_flick_starts_core_momentum_and_advances_without_more_pointer_input() {
        let (mut tree, scroll) = scrollable();
        let mut touch = WorkerTouchScroll::new();

        touch.pointer_down(&mut tree, 50.0, 80.0, PointerKind::Touch, 10.0);
        let _ = tree.commit_rendered_frame(10.0);
        touch.pointer_move(&mut tree, 50.0, 60.0, PointerKind::Touch, 20.0);
        let _ = tree.commit_rendered_frame(20.0);
        touch.pointer_move(&mut tree, 50.0, 20.0, PointerKind::Touch, 40.0);
        let _ = tree.commit_rendered_frame(40.0);
        touch.pointer_up(&mut tree, 50.0, 20.0, PointerKind::Touch, 45.0);
        let _ = tree.commit_rendered_frame(45.0);
        let (_, offset_at_release) = tree.element_get_scroll_offset(scroll);

        assert!(
            tree.has_pending_visual_work(),
            "release frame must keep the Worker awake while Core momentum is active",
        );
        let _ = tree.commit_rendered_frame(61.0);
        let (_, offset_after_momentum) = tree.element_get_scroll_offset(scroll);
        assert!(
            offset_after_momentum > offset_at_release,
            "Core momentum must keep moving after pointer input ends",
        );
    }

    #[test]
    fn releasing_after_a_pause_does_not_replay_an_old_drag_as_phantom_momentum() {
        let (mut tree, _scroll) = scrollable();
        let mut touch = WorkerTouchScroll::new();

        touch.pointer_down(&mut tree, 50.0, 80.0, PointerKind::Touch, 10.0);
        let _ = tree.commit_rendered_frame(10.0);
        touch.pointer_move(&mut tree, 50.0, 60.0, PointerKind::Touch, 20.0);
        let _ = tree.commit_rendered_frame(20.0);
        touch.pointer_move(&mut tree, 50.0, 20.0, PointerKind::Touch, 40.0);
        let _ = tree.commit_rendered_frame(40.0);

        touch.pointer_up(&mut tree, 50.0, 20.0, PointerKind::Touch, 500.0);
        let _ = tree.commit_rendered_frame(500.0);

        assert!(
            !tree.has_pending_visual_work(),
            "a finger held still beyond the velocity window must release at rest",
        );
    }
}
