//! build 時 codegen（Hayabusa ADR-0008）：`components/*.hybs` を生成 Rust にコンパイルし、
//! `$OUT_DIR/components_generated.rs` に各コンポーネントを `pub mod <name> { ... }` で束ねる。
//! `src/lib.rs` の `pub mod generated` がこれを `include!` する。
//!
//! クレートは自身のライブラリを build.rs から使えないため、パース／生成は build-dependency の
//! `hayabusa-codegen` クレートに置く（ADR-0008）。

use std::fs;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let components_dir = Path::new("components");

    // components/ の追加・変更で再生成する。
    println!("cargo:rerun-if-changed=components");
    println!("cargo:rerun-if-changed=build.rs");

    let mut modules = String::new();

    if components_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(components_dir)
            .expect("read components/")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "hybs").unwrap_or(false))
            .collect();
        entries.sort();

        for path in entries {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("component file stem")
                .to_string();
            println!("cargo:rerun-if-changed={}", path.display());

            let source = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let code = hayabusa_codegen::compile_hybs(&source, &name)
                .unwrap_or_else(|e| panic!("compile {}: {e}", path.display()));

            let module_file = Path::new(&out_dir).join(format!("{name}.rs"));
            fs::write(&module_file, code).expect("write generated module");

            modules.push_str(&format!(
                "pub mod {name} {{ include!(concat!(env!(\"OUT_DIR\"), \"/{name}.rs\")); }}\n"
            ));
        }
    }

    let agg = Path::new(&out_dir).join("components_generated.rs");
    fs::write(&agg, modules).expect("write components_generated.rs");
}
