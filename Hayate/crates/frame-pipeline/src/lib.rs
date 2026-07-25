//! Platform-free Latest-Wins Frame Pipeline (ADR-0157).
//!
//! This module owns admission, adjacent-frame replacement, lifecycle barriers, completion,
//! terminal failure, and overload observations. Execution adapters own synchronization and wake
//! mechanisms and drive the module through [`admit`](LatestWinsFramePipeline::admit),
//! [`start_next`](LatestWinsFramePipeline::start_next), and
//! [`complete_active`](LatestWinsFramePipeline::complete_active).

use std::collections::{HashSet, VecDeque};

use hayate_core::{CommittedFrame, LayerTopology, SceneSnapshot, ScrollCompositorInput};

/// Frame behavior required by the pipeline's latest-wins admission policy.
pub trait CoalescingFrame {
    /// Replace the current snapshot with `latest` while carrying forward every dirty or otherwise
    /// unapplied fact from the superseded frame.
    fn replace_with_latest(&mut self, latest: Self);
}

/// Execution-ready value frozen from one Core commit. It contains no execution, platform, surface,
/// or renderer resource.
#[derive(Debug, Clone)]
pub struct FrameSubmission {
    /// Immutable snapshot from the latest accepted Core commit.
    pub scene: SceneSnapshot,
    /// Latest layer topology plus dirty work accumulated from superseded frames.
    pub topology: LayerTopology,
    /// Latest scroll geometry with superseded content invalidations carried forward.
    pub scroll_inputs: Vec<ScrollCompositorInput>,
}

impl FrameSubmission {
    /// Freeze every rendering-visible fact from one committed frame into an owned value.
    pub fn from_committed_frame(frame: &CommittedFrame) -> Self {
        Self {
            scene: frame.snapshot().clone(),
            topology: frame.layer_topology().clone(),
            scroll_inputs: frame.scroll_inputs().to_vec(),
        }
    }
}

impl CoalescingFrame for FrameSubmission {
    fn replace_with_latest(&mut self, latest: Self) {
        let older = std::mem::replace(self, latest);
        let older_scroll_dirty: HashSet<_> = older
            .scroll_inputs
            .iter()
            .filter(|input| input.content_dirty)
            .map(|input| input.layer)
            .collect();
        self.topology.absorb_changes_from(&older.topology);
        for input in &mut self.scroll_inputs {
            input.content_dirty |= self.topology.content_changed().contains(&input.layer)
                || older_scroll_dirty.contains(&input.layer);
        }
    }
}

/// A frame is replaceable by a newer adjacent frame. A barrier is never replaced and prevents
/// frames on either side from being coalesced with each other.
#[derive(Debug, PartialEq, Eq)]
pub enum PipelineCommand<F, B> {
    Frame(F),
    Barrier(B),
}

/// Observable result of accepting a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Accepted,
    Coalesced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmitError {
    TerminalFailure,
}

/// Completion notifications must correspond to an active command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionError {
    NoActiveCommand,
}

/// Stable performance and failure facts exposed by the shared pipeline interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipelineObservation {
    /// Commands admitted before terminal failure, including commands later coalesced.
    pub accepted: u64,
    /// Incoming frames merged into an adjacent pending frame.
    pub coalesced: u64,
    /// Pending commands discarded at failure plus commands rejected after failure.
    pub dropped: u64,
    /// Whether an execution adapter currently owns one command.
    pub active: bool,
    /// Commands waiting behind the active command.
    pub pending: usize,
    /// Whether terminal failure has permanently stopped the pipeline.
    pub failure: bool,
}

/// Platform-free admission and completion policy. Execution adapters provide their own wake and
/// synchronization mechanism around this value.
pub struct LatestWinsFramePipeline<F, B> {
    pending: VecDeque<PipelineCommand<F, B>>,
    active: bool,
    accepted: u64,
    coalesced: u64,
    dropped: u64,
    failure: bool,
}

impl<F, B> Default for LatestWinsFramePipeline<F, B> {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            active: false,
            accepted: 0,
            coalesced: 0,
            dropped: 0,
            failure: false,
        }
    }
}

impl<F, B> LatestWinsFramePipeline<F, B>
where
    F: CoalescingFrame,
{
    /// Create an idle pipeline.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accept one frame or barrier without executing it.
    pub fn admit(&mut self, command: PipelineCommand<F, B>) -> Result<Admission, AdmitError> {
        if self.failure {
            self.dropped = self.dropped.saturating_add(1);
            return Err(AdmitError::TerminalFailure);
        }
        self.accepted = self.accepted.saturating_add(1);
        if let PipelineCommand::Frame(incoming) = command {
            if let Some(PipelineCommand::Frame(current)) = self.pending.back_mut() {
                current.replace_with_latest(incoming);
                self.coalesced = self.coalesced.saturating_add(1);
                return Ok(Admission::Coalesced);
            }
            self.pending.push_back(PipelineCommand::Frame(incoming));
        } else {
            self.pending.push_back(command);
        }
        Ok(Admission::Accepted)
    }

    /// Transfer the next command to an execution adapter and mark it active.
    pub fn start_next(&mut self) -> Option<PipelineCommand<F, B>> {
        if self.active || self.failure {
            return None;
        }
        let next = self.pending.pop_front()?;
        self.active = true;
        Some(next)
    }

    /// Notify the pipeline that its active command completed successfully.
    pub fn complete_active(&mut self) -> Result<(), CompletionError> {
        if !self.active {
            return Err(CompletionError::NoActiveCommand);
        }
        self.active = false;
        Ok(())
    }

    /// Latch a renderer or execution failure. Failure may arrive while a command is active or
    /// asynchronously while idle; both forms are terminal.
    pub fn fail(&mut self) {
        self.active = false;
        self.failure = true;
        self.dropped = self
            .dropped
            .saturating_add(self.pending.len().try_into().unwrap_or(u64::MAX));
        self.pending.clear();
    }

    /// Read stable overload and terminal-state facts.
    pub fn observation(&self) -> PipelineObservation {
        PipelineObservation {
            accepted: self.accepted,
            coalesced: self.coalesced,
            dropped: self.dropped,
            active: self.active,
            pending: self.pending.len(),
            failure: self.failure,
        }
    }
}
