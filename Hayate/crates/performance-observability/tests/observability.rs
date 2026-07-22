use hayate_performance_observability::{
    FrameCounters, FrameDeadline, PerformanceObservability, PerformancePhase,
};

#[test]
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
