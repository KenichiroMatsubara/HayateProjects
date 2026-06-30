//! ホスト可読の QR スキャナ capability ガード（ADR-0125）。iOS の `qr_scanner_encapsulation.rs` の
//! 鏡写し。本アプリ初の Rust↔Kotlin JNI seam を 1 モジュールに封じ込める。
//!
//! 1. **封じ込め**: Code Scanner を呼ぶ JNI（`jni::` の使用）は QR glue モジュール内にのみ現れてよい。
//!    audio の `hayate_ios_audio_*` ガードと同型に、汚い FFI を 1 か所へ寄せる。
//! 2. **契約遵守**: leaf の `AndroidQrScanner` が Core の [`QrScanner`] 契約を `#[cfg(target_os =
//!    "android")]` glue として実装することをソース走査で固定する。実機（NDK + Play services）が
//!    無くても成立する。

use std::fs;
use std::path::{Path, PathBuf};

/// JNI を使ってよい唯一のモジュール（QR スキャナ leaf）。
const QR_FILE: &str = "qr_scanner.rs";

/// QR glue のシーム内に留めるべき Rust↔Kotlin JNI の使用マーカー。
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
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn jni_is_confined_to_the_qr_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    assert!(!files.is_empty(), "expected to scan some source files");

    let mut violations = Vec::new();
    for file in &files {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == QR_FILE {
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
        "Rust↔Kotlin JNI (`{FORBIDDEN_MARKER}*`) must live only in `{QR_FILE}` \
         (route through the Core `QrScanner` contract); found:\n{}",
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
