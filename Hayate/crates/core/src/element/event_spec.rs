//! Event と DocumentEventKind — `proto/spec/event_kinds.json` から生成。
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../proto/generated/event_types.rs"
));
