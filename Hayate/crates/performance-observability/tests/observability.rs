use hayate_performance_observability::{
    FrameCounters, FrameDeadline, PerformanceObservability, PerformancePhase,
    DEFAULT_REPORT_INTERVAL_FRAMES,
};

#[test]
#[cfg(feature = "enabled")]
fn records_one_fixed_size_frame_report_for_the_shared_pipeline_vocabulary() {
    let observability = PerformanceObservability::new();
    let mut frame = observability.begin_frame(FrameDeadline::from_refresh_rate_hz(60));

    frame.record_phase(PerformancePhase::AppHost, 100);
    frame.record_phase(PerformancePhase::CoreCommit, 200);
    frame.record_phase(PerformancePhase::SceneLowering, 300);
    frame.record_phase(PerformancePhase::LayerPresentation, 400);
    frame.record_phase(PerformancePhase::RendererSubmit, 500);
    frame.record_phase(PerformancePhase::RendererPresent, 600);
    frame.set_counters(FrameCounters {
        nodes: 12,
        layers: 3,
        dirty_layers: 2,
        cache_hits: 4,
        cache_misses: 1,
        allocations: 0,
        cpu_resident_bytes: 8 * 1024 * 1024,
        gpu_resident_bytes: 24 * 1024 * 1024,
        resource_evictions: 2,
        resource_rebuild_cost: 4096,
    });
    frame.finish();

    let report = observability
        .latest_report()
        .expect("the completed frame is retained");
    assert_eq!(report.deadline_ns, 16_666_667);
    assert_eq!(report.total_phase_ns(), 2_100);
    assert!(!report.missed_deadline());
    assert_eq!(report.counters.layers, 3);
    assert_eq!(report.counters.cache_misses, 1);
    assert_eq!(report.counters.cpu_resident_bytes, 8 * 1024 * 1024);
    assert_eq!(report.counters.gpu_resident_bytes, 24 * 1024 * 1024);
    assert_eq!(report.counters.resource_evictions, 2);
    assert_eq!(report.counters.resource_rebuild_cost, 4096);
}

#[test]
fn periodic_summary_exposes_window_p95_long_frames_and_residency_for_matrix_runs() {
    let observability = PerformanceObservability::new();
    let deadline = FrameDeadline::from_refresh_rate_hz(60);
    for frame_index in 0..DEFAULT_REPORT_INTERVAL_FRAMES {
        let mut frame = observability.begin_frame(deadline);
        let total = if frame_index < 4 {
            40_000_000
        } else {
            1_000_000
        };
        frame.record_phase(PerformancePhase::RendererPresent, total);
        frame.set_counters(FrameCounters {
            cpu_resident_bytes: 10 + frame_index,
            gpu_resident_bytes: 20 + frame_index,
            resource_evictions: frame_index,
            resource_rebuild_cost: frame_index * 2,
            ..FrameCounters::default()
        });
        frame.finish();
    }

    let summary = observability
        .periodic_summary()
        .expect("summary at named interval");

    assert_eq!(summary.sample_count, DEFAULT_REPORT_INTERVAL_FRAMES as u64);
    assert_eq!(summary.frames_over_two_intervals, 4);
    assert_eq!(
        summary.phase_p95_ns[PerformancePhase::RendererPresent as usize],
        40_000_000
    );
    assert_eq!(summary.counters.cpu_resident_bytes, 69);
    assert_eq!(summary.counters.gpu_resident_bytes, 79);
    assert_eq!(summary.counters.resource_evictions, 59);
    assert_eq!(summary.counters.resource_rebuild_cost, 118);
}

#[test]
#[cfg(not(feature = "enabled"))]
fn production_default_does_not_retain_frame_reports() {
    let observability = PerformanceObservability::new();
    let mut frame = observability.begin_frame(FrameDeadline::from_refresh_rate_hz(60));
    frame.record_phase(PerformancePhase::AppHost, 100);
    frame.finish();

    assert!(!observability.is_enabled());
    assert_eq!(observability.latest_report(), None);
}
