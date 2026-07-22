//! Fixed-capacity frame observability shared by the App Host, Core, layer presentation and
//! renderer seams. Production builds keep the interface but compile its recording path out;
//! the profileable Android benchmark build enables the `enabled` feature.

use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Number of completed frames retained for a report. This is deliberately fixed so recording
/// never grows memory with the duration of a benchmark run.
pub const DEFAULT_RING_CAPACITY: usize = 120;
/// Number of completed frames between periodic Android summary emissions.
pub const DEFAULT_REPORT_INTERVAL_FRAMES: u64 = 60;
/// Conservative default used when a Platform Front has not supplied its display refresh rate.
pub const DEFAULT_REFRESH_RATE_HZ: u32 = 60;

/// The common phase vocabulary. Keep this ordered: reports store its timings in a fixed array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PerformancePhase {
    AppHost = 0,
    CoreCommit = 1,
    SceneLowering = 2,
    LayerPresentation = 3,
    RendererSubmit = 4,
    RendererPresent = 5,
}

impl PerformancePhase {
    pub const COUNT: usize = 6;

    fn trace_name(self) -> &'static str {
        match self {
            Self::AppHost => "Hayate.AppHost",
            Self::CoreCommit => "Hayate.CoreCommit",
            Self::SceneLowering => "Hayate.SceneLowering",
            Self::LayerPresentation => "Hayate.LayerPresentation",
            Self::RendererSubmit => "Hayate.RendererSubmit",
            Self::RendererPresent => "Hayate.RendererPresent",
        }
    }
}

/// Frame facts that remain useful across renderer implementations.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameCounters {
    pub nodes: u32,
    pub layers: u32,
    pub dirty_layers: u32,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub allocations: u32,
    pub cpu_resident_bytes: u64,
    pub gpu_resident_bytes: u64,
    pub resource_evictions: u64,
    pub resource_rebuild_cost: u64,
}

/// One refresh interval, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDeadline {
    nanos: u64,
}

impl FrameDeadline {
    pub fn from_refresh_rate_hz(refresh_rate_hz: u32) -> Self {
        assert!(refresh_rate_hz > 0, "refresh rate must be non-zero");
        Self {
            // A deadline must not be rounded down: 60Hz is 16,666,666.67ns, so the
            // observable frame budget is 16,666,667ns (ADR-0156's 16.67ms gate).
            nanos: 1_000_000_000_u64.div_ceil(u64::from(refresh_rate_hz)),
        }
    }

    pub fn nanos(self) -> u64 {
        self.nanos
    }
}

/// A completed frame report. It is `Copy` so reading a report never needs a heap allocation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameReport {
    pub deadline_ns: u64,
    pub phase_ns: [u64; PerformancePhase::COUNT],
    pub counters: FrameCounters,
}

impl FrameReport {
    pub fn total_phase_ns(&self) -> u64 {
        self.phase_ns.iter().sum()
    }

    pub fn missed_deadline(&self) -> bool {
        self.total_phase_ns() > self.deadline_ns
    }
}

struct ReportRing {
    reports: [FrameReport; DEFAULT_RING_CAPACITY],
    next: usize,
    len: usize,
    completed_frames: u64,
}

impl Default for ReportRing {
    fn default() -> Self {
        Self {
            reports: [FrameReport::default(); DEFAULT_RING_CAPACITY],
            next: 0,
            len: 0,
            completed_frames: 0,
        }
    }
}

impl ReportRing {
    fn push(&mut self, report: FrameReport) {
        self.reports[self.next] = report;
        self.next = (self.next + 1) % DEFAULT_RING_CAPACITY;
        self.len = (self.len + 1).min(DEFAULT_RING_CAPACITY);
        self.completed_frames += 1;
    }

    fn latest(&self) -> Option<FrameReport> {
        (self.len > 0)
            .then(|| self.reports[(self.next + DEFAULT_RING_CAPACITY - 1) % DEFAULT_RING_CAPACITY])
    }

    fn periodic_latest(&self) -> Option<FrameReport> {
        (self.completed_frames > 0
            && self
                .completed_frames
                .is_multiple_of(DEFAULT_REPORT_INTERVAL_FRAMES))
        .then(|| self.latest())
        .flatten()
    }
}

struct Inner {
    reports: Mutex<ReportRing>,
}

/// Deep module for the shared observation seam. It has one caller-facing operation per frame:
/// [`begin_frame`](Self::begin_frame). All mutable bookkeeping and bounded storage stays inside.
#[derive(Clone)]
pub struct PerformanceObservability {
    inner: Option<Arc<Inner>>,
}

impl Default for PerformanceObservability {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceObservability {
    /// Creates the production-default observer. Without the compile-time `enabled` feature this
    /// keeps no state and every record operation is a no-op.
    pub fn new() -> Self {
        if cfg!(feature = "enabled") {
            Self {
                inner: Some(Arc::new(Inner {
                    reports: Mutex::new(ReportRing::default()),
                })),
            }
        } else {
            Self { inner: None }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub fn begin_frame(&self, deadline: FrameDeadline) -> FrameObservation {
        FrameObservation {
            inner: self.inner.clone(),
            report: FrameReport {
                deadline_ns: deadline.nanos(),
                ..FrameReport::default()
            },
        }
    }

    pub fn latest_report(&self) -> Option<FrameReport> {
        self.inner.as_ref().and_then(|inner| {
            inner
                .reports
                .lock()
                .expect("observability mutex poisoned")
                .latest()
        })
    }

    /// Returns a report only at the named report interval. This prevents logcat summaries from
    /// becoming a per-frame allocation or I/O path.
    pub fn periodic_report(&self) -> Option<FrameReport> {
        self.inner.as_ref().and_then(|inner| {
            inner
                .reports
                .lock()
                .expect("observability mutex poisoned")
                .periodic_latest()
        })
    }
}

/// Stack-only observation for one frame. It performs no heap allocation while phases are
/// recorded. `finish` is the sole bounded-storage mutation.
pub struct FrameObservation {
    inner: Option<Arc<Inner>>,
    report: FrameReport,
}

impl FrameObservation {
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub fn record_phase(&mut self, phase: PerformancePhase, elapsed_ns: u64) {
        if self.inner.is_some() {
            self.report.phase_ns[phase as usize] = elapsed_ns;
        }
    }

    pub fn measure<T>(&mut self, phase: PerformancePhase, work: impl FnOnce() -> T) -> T {
        if self.inner.is_none() {
            return work();
        }
        let _trace = PerfettoSection::new(phase.trace_name());
        let started = Instant::now();
        let result = work();
        self.record_phase(
            phase,
            started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
        );
        result
    }

    pub fn set_counters(&mut self, counters: FrameCounters) {
        if self.inner.is_some() {
            self.report.counters = counters;
        }
    }

    pub fn finish(self) {
        if let Some(inner) = self.inner {
            inner
                .reports
                .lock()
                .expect("observability mutex poisoned")
                .push(self.report);
        }
    }
}

struct PerfettoSection;

impl PerfettoSection {
    fn new(name: &'static str) -> Self {
        #[cfg(target_os = "android")]
        android_trace_begin(name);
        #[cfg(not(target_os = "android"))]
        let _ = name;
        Self
    }
}

impl Drop for PerfettoSection {
    fn drop(&mut self) {
        #[cfg(target_os = "android")]
        unsafe {
            ATrace_endSection();
        }
    }
}

#[cfg(target_os = "android")]
#[link(name = "android")]
extern "C" {
    fn ATrace_beginSection(section_name: *const std::ffi::c_char);
    fn ATrace_endSection();
}

#[cfg(target_os = "android")]
fn android_trace_begin(name: &'static str) {
    let bytes = match name {
        "Hayate.AppHost" => b"Hayate.AppHost\0".as_ptr(),
        "Hayate.CoreCommit" => b"Hayate.CoreCommit\0".as_ptr(),
        "Hayate.SceneLowering" => b"Hayate.SceneLowering\0".as_ptr(),
        "Hayate.LayerPresentation" => b"Hayate.LayerPresentation\0".as_ptr(),
        "Hayate.RendererSubmit" => b"Hayate.RendererSubmit\0".as_ptr(),
        "Hayate.RendererPresent" => b"Hayate.RendererPresent\0".as_ptr(),
        _ => unreachable!("phase trace name is fixed"),
    };
    unsafe { ATrace_beginSection(bytes.cast()) };
}
