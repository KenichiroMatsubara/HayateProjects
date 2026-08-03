use hayate_adapter_android::generated::torimi_wire::{
    is_known_log_level, DemoManifest, LogBatch, LogEntry,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureCase {
    name: String,
    valid: bool,
    value: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureCorpus {
    log_entry: Vec<FixtureCase>,
    log_batch: Vec<FixtureCase>,
    demo_manifest: Vec<FixtureCase>,
}

#[test]
fn generated_rust_dtos_match_the_human_reviewed_fixture_expectations() {
    let corpus: FixtureCorpus = serde_json::from_str(include_str!(
        "../../../../../../Torimi/wire-contract/fixtures/parity.json"
    ))
    .unwrap();

    for case in corpus.log_entry {
        let accepted = serde_json::from_value::<LogEntry>(case.value).is_ok();
        assert_eq!(accepted, case.valid, "LogEntry: {}", case.name);
    }
    for case in corpus.log_batch {
        let accepted = serde_json::from_value::<LogBatch>(case.value).is_ok();
        assert_eq!(accepted, case.valid, "LogBatch: {}", case.name);
    }
    for case in corpus.demo_manifest {
        let accepted = serde_json::from_value::<DemoManifest>(case.value).is_ok();
        assert_eq!(accepted, case.valid, "DemoManifest: {}", case.name);
    }

    assert!(is_known_log_level("error"));
    assert!(!is_known_log_level("trace"));
}
