//! Host-side contract for the profileable Android performance tracer (issue #883).
//!
//! Gradle/ADB are intentionally not required by `cargo test`; this fixes the observable build
//! and collection path while device smoke remains a separate, explicit command.

use std::fs;
use std::path::PathBuf;

fn android_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = android_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn benchmark_variant_is_profileable_release_optimised_and_explicitly_instrumented() {
    let gradle = read("android-app/app/build.gradle.kts");
    let manifest = read("android-app/app/src/benchmark/AndroidManifest.xml");
    let cargo = read("Cargo.toml");

    assert!(
        gradle.contains("create(\"benchmark\")")
            && gradle.contains("initWith(getByName(\"release\"))")
    );
    assert!(gradle.contains("isDebuggable = false") && gradle.contains("isJniDebuggable = false"));
    assert!(gradle.contains("featureSpec.defaultAnd(arrayOf(\"performance-observability\"))"));
    assert!(manifest.contains("<profileable android:shell=\"true\""));
    assert!(cargo
        .contains("performance-observability = [\"hayate-performance-observability/enabled\"]"));
}

#[test]
fn fixed_vocabulary_is_emitted_at_a_bounded_summary_interval() {
    let app = read("src/app_tsubame.rs");
    let observability = read("../../../performance-observability/src/lib.rs");

    for phase in [
        "PerformancePhase::AppHost",
        "PerformancePhase::CoreCommit",
        "PerformancePhase::RendererSubmit",
    ] {
        assert!(app.contains(phase), "Android frame path records {phase}");
    }
    assert!(
        app.contains("target: \"HayatePerf\"") && app.contains("observability.periodic_report()")
    );
    assert!(
        observability.contains("DEFAULT_RING_CAPACITY")
            && observability.contains("DEFAULT_REPORT_INTERVAL_FRAMES")
    );
    assert!(
        observability.contains("LayerPresentation") && observability.contains("RendererPresent")
    );
}

#[test]
fn perfetto_and_adb_summary_are_collected_from_the_same_benchmark_run() {
    let script = read("../../../../scripts/collect-android-performance-report.sh");

    assert!(script.contains("am force-stop") && script.contains("am start -n"));
    assert!(
        script.contains("perfetto")
            && script.contains("ftrace/print")
            && script.contains("HayatePerf")
    );
    assert!(script.contains("adb pull") && script.contains("logcat -d"));
}
