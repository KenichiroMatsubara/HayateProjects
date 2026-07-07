//! Torimi Android ホストのバンドル源契約のホスト側検証（#532）。
//!
//! `app_tsubame` は device 専用（埋め込み Hermes / JSI が要る）でホストにはコンパイルされない
//! （ADR-0112）。そこで apk_packaging.rs と同じく、ソースを読んで「バンドル源を APK asset →
//! 実行時ネットワーク fetch に替えた／eval シームは不変」という契約を固定する。実描画は
//! ローカル実機で確認する（本 issue 外）。

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn app_tsubame_src() -> String {
    read_relative("src/app_tsubame.rs")
}

#[test]
fn host_fetches_the_bundle_over_the_network() {
    let src = app_tsubame_src();
    assert!(
        src.contains("bundle_source::fetch_from"),
        "run() must obtain the bundle via the network fetch source, driven by the resolved target (#532/#534)"
    );
}

#[test]
fn host_no_longer_reads_the_bundle_from_apk_assets() {
    let src = app_tsubame_src();
    assert!(
        !src.contains("asset_manager"),
        "the APK asset source is replaced by network fetch — no asset_manager read remains"
    );
    assert!(
        !src.contains("tsubame.js"),
        "the build-time embedded assets/tsubame.js path must be gone"
    );
}

#[test]
fn eval_seam_is_unchanged() {
    let src = app_tsubame_src();
    // 取得した JS ソースをそのまま Hermes に eval する経路（`new_hermes_app(make_bridge(tree..), bundle)`）
    // は不変。源（fetch）も reload も、この eval シームを呼び回すだけで作り替えない（#532/#533）。
    assert!(
        src.contains("new_hermes_app(make_bridge(tree.clone()), bundle)"),
        "the fetched source must still flow into the unchanged eval seam"
    );
}
