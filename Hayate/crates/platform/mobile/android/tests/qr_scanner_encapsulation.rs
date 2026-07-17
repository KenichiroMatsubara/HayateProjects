//! ホスト可読の QR スキャナ capability ガード（ADR-0125）。iOS の `qr_scanner_encapsulation.rs` の
//! 鏡写し。Rust↔Kotlin JNI を共通下地 1 モジュールに封じ込める。
//!
//! 1. **封じ込め**: `jni::` の直接使用は共通下地（`jni_bridge.rs`）内にのみ現れてよい。
//!    audio の `hayate_ios_audio_*` ガードと同型に、汚い FFI を 1 か所へ寄せる。QR スキャナ
//!    （`qr_scanner.rs`）・エラーオーバーレイ（`error_overlay.rs`）等の JNI leaf は全て
//!    `jni_bridge::with_activity_env` 経由で呼び、`jni::` を直接触らない。
//! 2. **契約遵守**: leaf の `AndroidQrScanner` が Core の [`QrScanner`] 契約を `#[cfg(target_os =
//!    "android")]` glue として実装することをソース走査で固定する。実機（NDK + Play services）が
//!    無くても成立する。

use std::fs;
use std::path::{Path, PathBuf};

/// `jni::` を直接使ってよい唯一のファイル（Rust↔Kotlin JNI の共通下地）。
const JNI_BRIDGE_FILE: &str = "jni_bridge.rs";

/// QR スキャナ leaf のファイル（`QrScanner` 契約の実装を固定する対象）。
const QR_FILE: &str = "qr_scanner.rs";

/// JNI の使用マーカー。共通下地（`JNI_BRIDGE_FILE`）以外に現れてはいけない。
const FORBIDDEN_MARKER: &str = "jni::";

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
fn jni_is_confined_to_the_bridge_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    assert!(!files.is_empty(), "expected to scan some source files");

    let mut violations = Vec::new();
    for file in &files {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == JNI_BRIDGE_FILE {
            continue;
        }
        let text = fs::read_to_string(file).expect("read source");
        for (lineno, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if line.contains(FORBIDDEN_MARKER) {
                violations.push(format!("{}:{}: {}", name, lineno + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Rust↔Kotlin JNI (`{FORBIDDEN_MARKER}*`) must live only in `{JNI_BRIDGE_FILE}` \
         (route through `jni_bridge::with_activity_env`); found:\n{}",
        violations.join("\n")
    );
}

#[test]
fn leaf_implements_the_core_qr_contract_under_cfg() {
    let qr = read_src(QR_FILE);
    assert!(
        qr.contains("impl QrScanner for AndroidQrScanner"),
        "the Android leaf must implement the Core `QrScanner` contract (canonical in Core)"
    );
    assert!(
        qr.contains("#[cfg(target_os = \"android\")]"),
        "the Code Scanner JNI glue must be gated to the android target (host build stays seam-only)"
    );
}
