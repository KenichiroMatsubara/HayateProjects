//! Enforcement guard (ADR-0069, #392): the platform soft-keyboard calls
//! `show_soft_input` / `hide_soft_input` may appear **only** in the IME bridge
//! module. Soft-keyboard visibility is decided once by core
//! (`ElementTree::drive_ime`) and reflected by `AndroidImeBridge`; any other
//! site calling the platform API directly would re-introduce the per-adapter
//! gating that let #392 (keyboard on every tap) land for Android only.
//!
//! This is a source-text scan, so it holds even for the `#[cfg(target_os =
//! "android")]` code that can't be compiled off-device.

use std::fs;
use std::path::Path;

/// The single module permitted to call the platform soft-input API.
const BRIDGE_FILE: &str = "ime_bridge.rs";

/// Platform IME calls that must stay behind the [`ImeBridge`] seam.
const FORBIDDEN: [&str; 2] = ["show_soft_input", "hide_soft_input"];

fn rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn soft_input_calls_are_confined_to_the_ime_bridge() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    assert!(!files.is_empty(), "expected to scan some source files");

    let mut violations = Vec::new();
    for file in &files {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == BRIDGE_FILE {
            continue;
        }
        let text = fs::read_to_string(file).expect("read source");
        for (lineno, line) in text.lines().enumerate() {
            // Skip comments so doc-references to the API don't trip the guard.
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for needle in FORBIDDEN {
                if line.contains(needle) {
                    violations.push(format!("{}:{}: {}", name, lineno + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "platform soft-input calls must live only in `{BRIDGE_FILE}` (route through \
         `ElementTree::drive_ime` + `AndroidImeBridge`); found:\n{}",
        violations.join("\n")
    );
}
