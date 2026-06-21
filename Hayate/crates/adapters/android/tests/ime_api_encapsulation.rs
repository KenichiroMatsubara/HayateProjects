//! 強制ガード（ADR-0069）: プラットフォームのソフトキーボード呼び出し
//! `show_soft_input` / `hide_soft_input` は IME ブリッジモジュール内にのみ現れてよい。
//! ソフトキーボードの表示可否は core（`ElementTree::drive_ime`）が一度だけ決定し
//! `AndroidImeBridge` が反映する。他の箇所がプラットフォーム API を直接呼ぶと、
//! アダプタごとのゲーティングが復活し「タップのたびにキーボード」が再発する。
//!
//! ソーステキスト走査なので、実機外でビルドできない `#[cfg(target_os = "android")]`
//! コードに対しても成立する。

use std::fs;
use std::path::Path;

/// プラットフォームのソフトインプット API を呼んでよい唯一のモジュール。
const BRIDGE_FILE: &str = "ime_bridge.rs";

/// [`ImeBridge`] のシーム内に留めるべきプラットフォーム IME 呼び出し。
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
        "platform soft-input calls must live only in `{BRIDGE_FILE}` (route through \
         `ElementTree::drive_ime` + `AndroidImeBridge`); found:\n{}",
        violations.join("\n")
    );
}
