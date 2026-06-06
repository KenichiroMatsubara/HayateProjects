use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let spec_dir = PathBuf::from(&manifest_dir).join("../../../proto/spec");
    let generated_dir = PathBuf::from(&manifest_dir).join("../../../proto/generated");

    for section in [
        "types",
        "enums",
        "opcodes",
        "style_tags",
        "event_kinds",
        "element_kinds",
        "unset_kinds",
        "modifier_keys",
    ] {
        let path = spec_dir.join(format!("{section}.json"));
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        spec_dir.join("manifest.json").display()
    );
    for file in ["protocol.rs", "dispatch.rs", "dom_style_mapper.rs"] {
        println!(
            "cargo:rerun-if-changed={}",
            generated_dir.join(file).display()
        );
    }
}
