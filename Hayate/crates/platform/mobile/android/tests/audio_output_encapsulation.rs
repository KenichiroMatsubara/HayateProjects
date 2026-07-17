//! ホスト可読の音声 capability ガード（ADR-0117 / #562）。
//!
//! 1. **封じ込め**: 発音を駆動する native AAudio FFI（`AAudio*`）は audio glue モジュール内に
//!    のみ現れてよい（`ime_bridge` のキーボード制御 FFI ガードと同型）。
//! 2. **契約遵守**: leaf の `AudioTrackOutput` が Core の [`AudioOutput`] 契約を `#[cfg(
//!    target_os = "android")]` glue として実装することをソース走査で固定する。実機（NDK +
//!    AAudio）が無くてもホストで成立する。

use std::fs;
use std::path::{Path, PathBuf};

/// 音声 native FFI を呼んでよい唯一のモジュール。
const AUDIO_FILE: &str = "audio_output.rs";

/// audio glue のシーム内に留めるべき発音駆動 native FFI（AAudio）のトークン。
const FORBIDDEN_PREFIX: &str = "AAudio";

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn read_src(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn aaudio_ffi_is_confined_to_the_audio_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    assert!(!files.is_empty(), "expected to scan some source files");

    let mut violations = Vec::new();
    for file in &files {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == AUDIO_FILE {
            continue;
        }
        let text = fs::read_to_string(file).expect("read source");
        for (lineno, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if line.contains(FORBIDDEN_PREFIX) {
                violations.push(format!("{}:{}: {}", name, lineno + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "sound-driving AAudio FFI (`{FORBIDDEN_PREFIX}*`) must live only in `{AUDIO_FILE}` \
         (route through the Core `AudioOutput` contract); found:\n{}",
        violations.join("\n")
    );
}

#[test]
fn leaf_implements_the_core_audio_contract_under_cfg() {
    let audio = read_src(AUDIO_FILE);
    assert!(
        audio.contains("impl AudioOutput for AudioTrackOutput"),
        "the Android leaf must implement the Core `AudioOutput` contract (canonical in Core)"
    );
    assert!(
        audio.contains("#[cfg(target_os = \"android\")]"),
        "the AudioTrack glue must be gated to the android target (host build stays seam-only)"
    );
}
