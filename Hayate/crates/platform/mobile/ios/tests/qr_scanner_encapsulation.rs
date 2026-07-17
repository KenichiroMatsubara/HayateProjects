//! ホスト可読の QR スキャナ capability ガード（ADR-0125）。`audio_output_encapsulation.rs` の鏡写し。
//!
//! 1. **封じ込め**: VisionKit を駆動する native FFI（`hayate_ios_qr_*`）は QR glue モジュール内に
//!    のみ現れてよい（audio の `hayate_ios_audio_*` ガードと同型）。
//! 2. **契約遵守**: leaf の `IosQrScanner` が Core の [`QrScanner`] 契約を `#[cfg(target_os = "ios")]`
//!    glue として実装することをソース走査で固定する。実機（iOS SDK + VisionKit）が無くても成立する。

use std::fs;
use std::path::{Path, PathBuf};

/// QR native FFI を呼んでよい唯一のモジュール。
const QR_FILE: &str = "qr_scanner.rs";

/// QR glue のシーム内に留めるべき VisionKit 駆動 FFI の接頭辞。
const FORBIDDEN_PREFIX: &str = "hayate_ios_qr_";

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
fn visionkit_ffi_is_confined_to_the_qr_module() {
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
            if line.contains(FORBIDDEN_PREFIX) {
                violations.push(format!("{}:{}: {}", name, lineno + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "VisionKit-driving FFI (`{FORBIDDEN_PREFIX}*`) must live only in `{QR_FILE}` \
         (route through the Core `QrScanner` contract); found:\n{}",
        violations.join("\n")
    );
}

#[test]
fn leaf_implements_the_core_qr_contract_under_cfg() {
    let qr = read_src(QR_FILE);
    assert!(
        qr.contains("impl QrScanner for IosQrScanner"),
        "the iOS leaf must implement the Core `QrScanner` contract (canonical in Core)"
    );
    assert!(
        qr.contains("#[cfg(target_os = \"ios\")]"),
        "the VisionKit glue must be gated to the ios target (host build stays seam-only)"
    );
}
