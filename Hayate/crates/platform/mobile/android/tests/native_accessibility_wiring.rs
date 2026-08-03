//! Android Native Accessibility leaf wiring contract (#919).

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn both_android_entry_paths_drive_the_shared_session_at_frame_boundaries() {
    for path in ["src/app.rs", "src/app_tsubame.rs"] {
        let src = read_relative(path);
        assert!(src.contains("mount_android_accessibility"), "{path}");
        assert!(src.contains("drain_before_frame"), "{path}");
        assert!(src.contains("update_after_present"), "{path}");
        assert!(src.contains("set_base_dpr"), "{path}");
        assert!(src.contains("destroy_android_accessibility"), "{path}");
    }
}

#[test]
fn game_activity_mounts_a_real_virtual_view_provider() {
    let kotlin = read_relative(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/AndroidAccessibilityBridge.kt",
    );
    assert!(kotlin.contains("AccessibilityNodeProvider"));
    assert!(kotlin.contains("createAccessibilityNodeInfo"));
    assert!(kotlin.contains("performAction"));
    assert!(kotlin.contains("ACTION_SET_TEXT"));
    assert!(kotlin.contains("ACTION_SHOW_ON_SCREEN"));
}

#[test]
fn activation_actions_focus_and_failure_categories_cross_only_the_leaf_boundary() {
    let rust = read_relative("src/android_accessibility.rs");
    let jni = read_relative("src/jni_bridge.rs");
    let activity = read_relative(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/MainActivity.kt",
    );

    assert!(rust.contains("NativeAccessibilitySession"));
    assert!(rust.contains("NativeAccessibilityMountFailure"));
    assert!(rust.contains("accesskit_consumer"));
    assert!(
        !rust.contains("baseline:"),
        "the leaf must not duplicate the shared session baseline"
    );
    assert!(jni.contains("nativeAccessibilityActivate"));
    assert!(jni.contains("nativeAccessibilityAction"));
    assert!(activity.contains("onWindowFocusChanged"));
    assert!(activity.contains("setContainerFocused"));
}
