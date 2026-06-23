//! `platform/` grouping doctrine のホスト可読契約（ADR-0117 / issue #454）。
//!
//! capability を「全 platform 共通 / family 共通 / leaf 固有」の三段階へ振り分ける規律と、
//! その枠（`platform/common` ・ `platform/desktop`）を構造として固定する。Rust サンドボックス
//! では desktop leaf を実体化できず、doctrine は本来ドキュメントなので、`*_packaging` /
//! `*_encapsulation` と同じくソース／ドキュメント走査でフレームと doctrine を pin する。
//!
//! 本 crate（mobile Family Adapter）は doctrine に参加する唯一の既存 Family Adapter なので、
//! 兄弟の枠（`../common` ・ `../desktop`）と doctrine 正本（`../README.md`）をここから検証する。

use std::fs;
use std::path::PathBuf;

/// `crates/platform/` ルート（本 crate manifest の親ディレクトリ）。
fn platform_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("mobile crate sits under crates/platform/")
        .to_path_buf()
}

#[test]
fn desktop_family_frame_exists_without_a_capability_trait() {
    let desktop = platform_root().join("desktop");
    assert!(
        desktop.is_dir(),
        "desktop family frame ディレクトリが存在しなければならない（前払いの枠）"
    );
    // 枠のみ。desktop は leaf 0 なので crate 化（Cargo.toml/src）も capability trait 定義もしない。
    // trait は最初の desktop leaf 着手時に Core へ足す（空 facade / 空 trait を先置きしない）。
    assert!(
        !desktop.join("Cargo.toml").exists(),
        "desktop は枠であって crate ではない（空 facade を先置きしない・ADR-0117）"
    );
    assert!(
        !desktop.join("src").exists(),
        "desktop は leaf も capability trait も持たない（src 無し）"
    );
    let readme =
        fs::read_to_string(desktop.join("README.md")).expect("desktop/README.md が doctrine を述べる");
    assert!(
        readme.contains("leaf 0") || readme.contains("leaf が 0"),
        "desktop README は leaf 0 の枠であることを明記する"
    );
    assert!(
        readme.contains("Core"),
        "desktop README は capability trait を最初の leaf 着手時に Core へ足す規律を述べる"
    );
}

#[test]
fn common_tier_frame_exists_without_a_capability_trait() {
    let common = platform_root().join("common");
    assert!(
        common.is_dir(),
        "全 platform 共通段の枠 platform/common/ が存在しなければならない"
    );
    // common も枠のみ。capability の契約は Core 所有なので、ここに trait を切らない。
    // 全 platform 共通の capability 実装が現れたときに初めて中身が入る（昇格は 2 実装から）。
    assert!(
        !common.join("Cargo.toml").exists(),
        "common は枠であって crate ではない（昇格する実装が揃うまで空 crate を作らない）"
    );
    assert!(
        !common.join("src").exists(),
        "common は capability trait を持たない（契約の正本は Core）"
    );
    let readme =
        fs::read_to_string(common.join("README.md")).expect("common/README.md が枠の役割を述べる");
    assert!(
        readme.contains("全 platform 共通") && readme.contains("Core"),
        "common README は『全 platform 共通段・契約は Core』を明記する"
    );
}

#[test]
fn doctrine_documents_the_three_tier_sorting_rule() {
    let doctrine = fs::read_to_string(platform_root().join("README.md"))
        .expect("platform/README.md が grouping doctrine の正本");
    // 三段階の振り分け規則（全 platform 共通 / family 共通 / leaf 固有）が明文化される。
    assert!(
        doctrine.contains("全 platform 共通") && doctrine.contains("platform/common"),
        "doctrine は最上段（全 platform 共通＝platform/common）を述べる"
    );
    assert!(
        doctrine.contains("family 共通")
            && doctrine.contains("platform/mobile")
            && doctrine.contains("platform/desktop"),
        "doctrine は中段（family 共通＝platform/mobile・platform/desktop）を述べる"
    );
    assert!(
        doctrine.contains("leaf 固有"),
        "doctrine は最下段（leaf 固有）を述べる"
    );
}

#[test]
fn doctrine_classifies_the_flutter_rn_taxonomy_examples() {
    let doctrine = fs::read_to_string(platform_root().join("README.md")).unwrap();
    // Flutter plugin / RN TurboModule の taxonomy から写した分類例。借りるのは
    // カタログ（どの機能がどの段に属するか）であって機構ではない。
    for capability in ["audio", "clipboard", "notification", "haptics", "file picker"] {
        assert!(
            doctrine.contains(capability),
            "doctrine は taxonomy 分類例として `{capability}` を含める"
        );
    }
    assert!(
        doctrine.contains("Flutter") && doctrine.contains("RN"),
        "doctrine は Flutter plugin / RN TurboModule の taxonomy 由来を明示する"
    );
}

#[test]
fn doctrine_states_the_three_disciplines() {
    let doctrine = fs::read_to_string(platform_root().join("README.md")).unwrap();
    // (1) 契約の正本は常に Core。
    assert!(
        doctrine.contains("契約") && doctrine.contains("Core"),
        "doctrine は『契約の正本は常に Core』を述べる"
    );
    // (2) 共通 API への昇格は原則 2 実装が揃ってから。
    assert!(
        doctrine.contains("2 実装") && doctrine.contains("昇格"),
        "doctrine は『昇格は 2 実装から』の規律を述べる"
    );
    // (3) 借りるのは taxonomy のみ、機構（channel / bridge）は借りない。
    assert!(
        doctrine.contains("taxonomy") && doctrine.contains("機構"),
        "doctrine は『taxonomy のみ借りる・機構（channel/bridge）は借りない』を述べる"
    );
}
