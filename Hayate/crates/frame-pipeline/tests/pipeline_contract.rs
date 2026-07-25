use std::collections::BTreeSet;

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{
    Color, CommittedFrame, ElementId, ElementKind, ElementTree, ScrollCompositorInput,
};
use hayate_frame_pipeline::{
    Admission, AdmitError, CoalescingFrame, FrameSubmission, LatestWinsFramePipeline,
    PipelineCommand,
};

#[derive(Debug, PartialEq, Eq)]
struct TestFrame {
    snapshot: u64,
    dirty: BTreeSet<u64>,
}

#[derive(Debug, PartialEq, Eq)]
enum TestBarrier {
    Resize,
    SurfaceLost,
    Rebuild,
}

impl TestFrame {
    fn new(snapshot: u64, dirty: impl IntoIterator<Item = u64>) -> Self {
        Self {
            snapshot,
            dirty: dirty.into_iter().collect(),
        }
    }
}

impl CoalescingFrame for TestFrame {
    fn replace_with_latest(&mut self, latest: Self) {
        let older_dirty = std::mem::take(&mut self.dirty);
        *self = latest;
        self.dirty.extend(older_dirty);
    }
}

#[test]
fn active_frame_and_many_new_frames_stay_bounded_to_one_coalesced_pending_frame() {
    let mut pipeline = LatestWinsFramePipeline::<TestFrame, ()>::new();
    assert_eq!(
        pipeline.admit(PipelineCommand::Frame(TestFrame::new(0, [0]))),
        Ok(Admission::Accepted)
    );
    assert_eq!(
        pipeline.start_next(),
        Some(PipelineCommand::Frame(TestFrame::new(0, [0])))
    );

    for snapshot in 1..=50 {
        pipeline
            .admit(PipelineCommand::Frame(TestFrame::new(snapshot, [snapshot])))
            .unwrap();
    }

    let observation = pipeline.observation();
    assert!(observation.active);
    assert_eq!(observation.pending, 1);
    assert_eq!(observation.accepted, 51);
    assert_eq!(observation.coalesced, 49);
    assert_eq!(observation.dropped, 0);
    assert!(!observation.failure);

    pipeline.complete_active().unwrap();
    assert_eq!(
        pipeline.start_next(),
        Some(PipelineCommand::Frame(TestFrame::new(50, 1..=50)))
    );
}

fn committed_frame_with_content_dirty(dirty: &[u64]) -> CommittedFrame {
    let mut tree = ElementTree::new();
    let root = tree.element_create(u64::MAX, ElementKind::View);
    tree.set_root(root);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    for &raw in dirty {
        let layer = tree.element_create(raw, ElementKind::View);
        tree.element_append_child(root, layer);
        tree.element_set_transform(layer, Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    }
    let _initial = tree.commit_rendered_frame(0.0);
    for &raw in dirty {
        tree.element_set_style(
            hayate_core::ElementId::from_u64(raw),
            &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
        );
    }
    tree.commit_rendered_frame(16.0)
}

#[test]
fn committed_frames_keep_the_latest_snapshot_and_union_superseded_dirty_work() {
    let older = FrameSubmission::from_committed_frame(&committed_frame_with_content_dirty(&[1]));
    let latest =
        FrameSubmission::from_committed_frame(&committed_frame_with_content_dirty(&[2, 3]));
    let latest_snapshot_len = latest.scene.len();
    let mut pipeline = LatestWinsFramePipeline::<FrameSubmission, ()>::new();

    pipeline.admit(PipelineCommand::Frame(older)).unwrap();
    pipeline.admit(PipelineCommand::Frame(latest)).unwrap();

    let Some(PipelineCommand::Frame(merged)) = pipeline.start_next() else {
        panic!("a coalesced frame must be executable");
    };
    let dirty: BTreeSet<u64> = merged
        .topology
        .content_changed()
        .iter()
        .map(|id| id.to_u64())
        .collect();
    assert_eq!(merged.scene.len(), latest_snapshot_len);
    assert_eq!(dirty, BTreeSet::from([1, 2, 3]));
}

fn committed_frame_with_transform_dirty(dirty: &[u64]) -> CommittedFrame {
    let mut tree = ElementTree::new();
    let root = tree.element_create(u64::MAX, ElementKind::View);
    tree.set_root(root);
    for &raw in dirty {
        let layer = tree.element_create(raw, ElementKind::View);
        tree.element_append_child(root, layer);
        tree.element_set_transform(layer, Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    }
    let _initial = tree.commit_rendered_frame(0.0);
    for &raw in dirty {
        tree.element_set_transform(
            ElementId::from_u64(raw),
            Some([1.0, 0.0, 0.0, 1.0, raw as f64, 0.0]),
        );
    }
    tree.commit_rendered_frame(16.0)
}

#[test]
fn committed_frames_union_transform_dirty_work() {
    let mut pipeline = LatestWinsFramePipeline::<FrameSubmission, ()>::new();
    pipeline
        .admit(PipelineCommand::Frame(
            FrameSubmission::from_committed_frame(&committed_frame_with_transform_dirty(&[1])),
        ))
        .unwrap();
    pipeline
        .admit(PipelineCommand::Frame(
            FrameSubmission::from_committed_frame(&committed_frame_with_transform_dirty(&[2])),
        ))
        .unwrap();

    let Some(PipelineCommand::Frame(merged)) = pipeline.start_next() else {
        panic!("a coalesced frame must be executable");
    };
    let dirty: BTreeSet<u64> = merged
        .topology
        .transform_changed()
        .iter()
        .map(|id| id.to_u64())
        .collect();
    assert_eq!(dirty, BTreeSet::from([1, 2]));
}

#[test]
fn committed_frames_keep_old_scroll_dirty_on_the_latest_geometry() {
    let layer = ElementId::from_u64(7);
    let mut old = FrameSubmission::from_committed_frame(&committed_frame_with_content_dirty(&[7]));
    old.scroll_inputs.push(ScrollCompositorInput {
        layer,
        absolute_top: 0.0,
        viewport_height: 100.0,
        scroll_offset: 0.0,
        max_scroll_offset: 500.0,
        scroll_affine: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        content_dirty: true,
    });
    let mut latest =
        FrameSubmission::from_committed_frame(&committed_frame_with_content_dirty(&[]));
    latest.scroll_inputs.push(ScrollCompositorInput {
        layer,
        absolute_top: 0.0,
        viewport_height: 100.0,
        scroll_offset: 40.0,
        max_scroll_offset: 500.0,
        scroll_affine: [1.0, 0.0, 0.0, 1.0, 0.0, -40.0],
        content_dirty: false,
    });
    let mut pipeline = LatestWinsFramePipeline::<FrameSubmission, ()>::new();
    pipeline.admit(PipelineCommand::Frame(old)).unwrap();
    pipeline.admit(PipelineCommand::Frame(latest)).unwrap();

    let Some(PipelineCommand::Frame(merged)) = pipeline.start_next() else {
        panic!("a coalesced frame must be executable");
    };
    assert_eq!(merged.scroll_inputs[0].scroll_offset, 40.0);
    assert_eq!(merged.scroll_inputs[0].scroll_affine[5], -40.0);
    assert!(merged.scroll_inputs[0].content_dirty);
}

#[test]
fn terminal_failure_drops_pending_work_and_rejects_every_future_frame() {
    let mut pipeline = LatestWinsFramePipeline::<TestFrame, ()>::new();
    pipeline
        .admit(PipelineCommand::Frame(TestFrame::new(1, [1])))
        .unwrap();
    assert!(pipeline.start_next().is_some());
    pipeline
        .admit(PipelineCommand::Frame(TestFrame::new(2, [2])))
        .unwrap();

    pipeline.fail();

    let failed = pipeline.observation();
    assert!(!failed.active);
    assert_eq!(failed.pending, 0);
    assert_eq!(failed.dropped, 1);
    assert!(failed.failure);
    assert_eq!(
        pipeline.admit(PipelineCommand::Frame(TestFrame::new(3, [3]))),
        Err(AdmitError::TerminalFailure)
    );
    assert_eq!(pipeline.observation().dropped, 2);
    assert!(pipeline.start_next().is_none());
}

#[test]
fn lifecycle_barriers_preserve_order_and_prevent_cross_boundary_frame_replacement() {
    let mut pipeline = LatestWinsFramePipeline::<TestFrame, TestBarrier>::new();
    let commands = [
        PipelineCommand::Frame(TestFrame::new(1, [1])),
        PipelineCommand::Barrier(TestBarrier::Resize),
        PipelineCommand::Frame(TestFrame::new(2, [2])),
        PipelineCommand::Barrier(TestBarrier::SurfaceLost),
        PipelineCommand::Frame(TestFrame::new(3, [3])),
        PipelineCommand::Barrier(TestBarrier::Rebuild),
        PipelineCommand::Frame(TestFrame::new(4, [4])),
        PipelineCommand::Frame(TestFrame::new(5, [5])),
    ];
    for command in commands {
        pipeline.admit(command).unwrap();
    }

    let mut drained = Vec::new();
    while let Some(command) = pipeline.start_next() {
        drained.push(command);
        pipeline.complete_active().unwrap();
    }

    assert_eq!(
        drained,
        vec![
            PipelineCommand::Frame(TestFrame::new(1, [1])),
            PipelineCommand::Barrier(TestBarrier::Resize),
            PipelineCommand::Frame(TestFrame::new(2, [2])),
            PipelineCommand::Barrier(TestBarrier::SurfaceLost),
            PipelineCommand::Frame(TestFrame::new(3, [3])),
            PipelineCommand::Barrier(TestBarrier::Rebuild),
            PipelineCommand::Frame(TestFrame::new(5, [4, 5])),
        ]
    );
}

#[test]
fn terminal_failure_can_latch_while_the_pipeline_is_idle() {
    let mut pipeline = LatestWinsFramePipeline::<TestFrame, ()>::new();

    pipeline.fail();

    assert!(pipeline.observation().failure);
    assert_eq!(
        pipeline.admit(PipelineCommand::Frame(TestFrame::new(1, [1]))),
        Err(AdmitError::TerminalFailure)
    );
}
