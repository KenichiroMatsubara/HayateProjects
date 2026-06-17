use std::env;
use std::path::PathBuf;

use hayate_proto_generator::generate_all;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = PathBuf::from(manifest_dir);
    let spec_dir = root.join("../spec");
    let out_dir = root.join("../generated");

    generate_all(&spec_dir, &out_dir);
    println!(
        "Generated Hayate/proto/generated/{{protocol,codec,dispatch,dom_style_mapper,event_types,pseudo_state_tables,element_kind_tables}}.rs"
    );
}
