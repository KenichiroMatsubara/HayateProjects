//! 強制ガード（ADR-0069 / ADR-0114）: プラットフォームのソフトキーボード制御 FFI
//! `hayate_ios_set_keyboard_visible`（Swift ホストが `becomeFirstResponder` /
//! `resignFirstResponder` に写す）は IME ブリッジモジュール内にのみ現れてよい。
//! ソフトキーボードの表示可否は core（`ElementTree::drive_ime`）が一度だけ決定し
//! `IosImeBridge` が反映する。他の箇所がキーボード制御を直接呼ぶと、アダプタごとの
//! ゲーティングが復活し「タップのたびにキーボード」が再発する（Android の
//! `show_soft_input` / `hide_soft_input` ガードと同型）。
//!
//! ソーステキスト走査なので、Mac/SDK 無しでビルドできない `#[cfg(target_os = "ios")]`
//! コードに対しても成立する。

use std::fs;
use std::path::Path;

/// プラットフォームのキーボード制御 FFI を呼んでよい唯一のモジュール。
const BRIDGE_FILE: &str = "ime_bridge.rs";

/// [`ImeBridge`] のシーム内に留めるべきプラットフォームキーボード制御呼び出し。
const FORBIDDEN: [&str; 1] = ["hayate_ios_set_keyboard_visible"];

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
fn keyboard_control_calls_are_confined_to_the_ime_bridge() {
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
            // コメントはスキップし、API へのドキュメント参照でガードが誤発火しないようにする。
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
        "platform keyboard-control calls must live only in `{BRIDGE_FILE}` (route through \
         `ElementTree::drive_ime` + `IosImeBridge`); found:\n{}",
        violations.join("\n")
    );
}
