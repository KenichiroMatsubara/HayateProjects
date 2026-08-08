//! Torimi Android host's fatal JS-frame boundary.
//!
//! Hermes and the JSI/C++ bridge only compile for Android, so host CI cannot execute this boundary
//! directly. As with the other device wiring tests in this crate, this test locks down the actual
//! C++ catch path and its Rust-to-native-overlay reporting route. The connected-device regression
//! loop in `scripts/repro-torimi-solid-sort-crash.sh` exercises the complete runtime path.

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn every_fatal_pump_frame_exception_is_contained_and_shown_on_the_native_overlay() {
    let cpp = read_relative("cpp/hermes_app.cpp");
    let pump_start = cpp
        .find("void HermesApp::pump_frame")
        .expect("Hermes pump_frame definition");
    let pump_end = cpp[pump_start..]
        .find("void HermesApp::request_redraw")
        .map(|offset| pump_start + offset)
        .expect("request_redraw definition after pump_frame");
    let pump = &cpp[pump_start..pump_end];

    assert!(
        pump.contains("catch (...)"),
        "the top-level JS frame boundary must contain unknown C++ exceptions that are able to \
         unwind to the host instead of letting them kill Torimi"
    );
    assert_eq!(
        pump.matches("report_fatal_frame_error").count(),
        3,
        "JSError, std::exception, and unknown-exception branches must all report the fatal frame \
         failure before disabling the runtime"
    );

    let bridge = read_relative("src/hermes_bridge.rs");
    assert!(
        bridge.contains("fn report_fatal_frame_error")
            && bridge.contains("crate::error_overlay::show_error")
            && bridge.contains("category=runtime-frame"),
        "the C++ boundary report must reach the GPU-independent native Android error overlay"
    );
}

#[test]
fn every_js_execution_entry_uses_the_android_application_class_loader() {
    let cpp = read_relative("cpp/hermes_app.cpp");

    assert!(
        cpp.contains("#include <fbjni/detail/Environment.h>"),
        "the embedded Hermes host must use FBJNI's application-class-loader scope"
    );
    assert!(
        cpp.matches("ThreadScope::WithClassLoader").count() >= 3,
        "bundle eval, frame pumping, and redraw callbacks can all invoke Hermes Intl; each native \
         JS entry must re-enter through FBJNI's application class loader"
    );
}
