//! iOS Native Accessibility leaf wiring contract (#921).

use std::fs;
use std::path::PathBuf;

fn read_relative(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn each_uiview_surface_mounts_a_real_accesskit_target_on_the_shared_session() {
    let cargo = read_relative("Cargo.toml");
    let leaf = read_relative("src/accessibility.rs");
    let app = read_relative("src/app.rs");
    let view = read_relative("ios-app/Hayate/HayateView.swift");

    assert!(cargo.contains("accesskit_ios"));
    assert!(leaf.contains("accesskit_ios::SubclassingAdapter"));
    assert!(leaf.contains("NativeAccessibilityTarget"));
    assert!(leaf.contains("NativeAccessibilitySession::new"));
    assert!(app.contains("mount_ios_accessibility"));
    assert!(view.contains("Unmanaged.passUnretained(self).toOpaque()"));
}

#[test]
fn voiceover_activation_publishes_the_full_tree_after_the_next_presented_frame() {
    let leaf = read_relative("src/accessibility.rs");
    let app = read_relative("src/app.rs");

    assert!(leaf.contains("handle.activate()"));
    assert!(leaf.contains("request_initial_tree(&mut self) -> Option<TreeUpdate>"));
    assert!(
        leaf.contains("None\n    }"),
        "activation must not synchronously borrow Core"
    );
    let drain = app
        .find("drain_before_frame")
        .expect("frame must drain the mailbox");
    let commit = app
        .find("commit_rendered_frame")
        .expect("frame must commit Core");
    let present = app
        .find("render_frame(&frame)")
        .expect("frame must present");
    let update = app
        .find("update_after_present")
        .expect("frame must publish the committed accessibility tree");
    assert!(drain < commit && commit < present && present < update);
}

#[test]
fn content_scale_changes_only_enter_accessibility_through_the_shared_root_dpr_seam() {
    let app = read_relative("src/app.rs");
    let leaf = read_relative("src/accessibility.rs");

    assert!(app.contains("set_base_dpr(app.content_scale as f64)"));
    assert!(
        !leaf.contains("set_transform") && !leaf.contains("set_bounds"),
        "the iOS leaf must not scale individual accessibility nodes"
    );
}

#[test]
fn scene_focus_restoration_republishes_the_container_without_clearing_core_focus() {
    let view = read_relative("ios-app/Hayate/HayateView.swift");

    let resign = view
        .split("func onResignActive()")
        .nth(1)
        .and_then(|body| body.split('}').next())
        .expect("resign-active body");
    assert!(
        !resign.contains("hayate_ios") && !resign.contains("resignFirstResponder"),
        "scene blur must not clear the Core element focus or the edit focus"
    );

    let active = view
        .split("func onBecomeActive()")
        .nth(1)
        .and_then(|body| body.split('}').next())
        .expect("become-active body");
    assert!(active.contains("UIAccessibility.post"));
    assert!(active.contains(".screenChanged"));
}

#[test]
fn activation_during_uiview_mount_is_deferred_until_the_session_handle_exists() {
    let leaf = read_relative("src/accessibility.rs");

    assert!(leaf.contains("pending_activation"));
    let adapter = leaf
        .find("SubclassingAdapter::new")
        .expect("real adapter construction");
    let session = leaf
        .find("NativeAccessibilitySession::new")
        .expect("shared session construction");
    let replay = leaf
        .rfind("handle.activate()")
        .expect("deferred activation replay");
    assert!(adapter < session && session < replay);
}

#[test]
fn uikit_actions_cross_the_leaf_as_accesskit_requests_without_platform_mapping() {
    let leaf = read_relative("src/accessibility.rs");

    assert!(leaf.contains("impl ActionHandler for Action"));
    assert!(leaf.contains("handle.action(request)"));
    for duplicated_mapping in [
        "Action::Focus",
        "Action::Click",
        "Action::SetValue",
        "Action::ScrollIntoView",
    ] {
        assert!(
            !leaf.contains(duplicated_mapping),
            "Core owns action mapping, not the iOS leaf: {duplicated_mapping}"
        );
    }
}

#[test]
fn surface_recreation_drops_the_old_target_and_mount_failure_keeps_rendering() {
    let leaf = read_relative("src/accessibility.rs");
    let app = read_relative("src/app.rs");
    let view = read_relative("ios-app/Hayate/HayateView.swift");

    assert!(app.contains("drop(Box::from_raw(app as *mut IosApp))"));
    assert!(view.contains("hayate_ios_app_free(app)"));
    assert!(view.contains("app = nil"));
    assert!(app.contains("native-accessibility platform={} category={}"));
    assert!(app.contains("Err(failure) =>"));
    assert!(
        app.contains("None"),
        "mount failure disables only accessibility"
    );
    assert!(!leaf.contains("baseline:"));
    assert!(!leaf.contains("HashMap"));
}
