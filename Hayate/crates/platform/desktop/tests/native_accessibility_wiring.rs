use std::fs;
use std::path::PathBuf;

fn read(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn desktop_window_mounts_the_real_accesskit_target_on_the_shared_session() {
    let cargo = read("Cargo.toml");
    let adapter = read("src/accessibility.rs");
    let front = read("src/lib.rs");

    assert!(cargo.contains("accesskit_winit"));
    assert!(adapter.contains("accesskit_winit::Adapter"));
    assert!(adapter.contains("NativeAccessibilityTarget"));
    assert!(adapter.contains("update_if_active"));
    assert!(front.contains("mount_desktop_accessibility"));
    assert!(front.contains("process_window_event"));
    assert!(front.contains("set_native_accessibility_base_dpr"));
}
