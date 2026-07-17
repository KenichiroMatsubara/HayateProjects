//! `platform/` grouping doctrine のホスト可読契約（ADR-0117 / issue #454）。
//!
//! capability を「全 platform 共通 / family 共通 / leaf 固有」の三段階へ振り分ける規律と、
//! その枠（`platform/common`）と desktop の実態を構造として固定する。`platform/common` は
//! いまも空の枠（capability 実装が 2 つ揃うまで crate 化しない）。`platform/desktop` は
//! ADR-0118 で windowing leaf（`hayate-platform-desktop`）に着手して実 crate になったが、
//! capability facade（audio 等）は依然 0 で「契約の正本は Core・空 facade を先置きしない」
//! という ADR-0117 の芯は維持する。doctrine は本来ドキュメントなので、`*_packaging` /
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
fn desktop_is_a_windowing_leaf_crate_without_a_capability_facade() {
    // ADR-0118 で desktop は **最初の windowing leaf**（`hayate-platform-desktop`）に着手した。
    // よって desktop はもはや空の枠ではなく、winit window + vello/wgpu Surface + App Host tick を
    // 束ねる実 crate である。ただし ADR-0117 の芯の規律 —— **capability facade（audio 等）は
    // 先置きしない・契約（trait）の正本は常に Core** —— は維持される。この doctrine テストは
    // 「crate 化した実態」と「capability facade は依然 0」の両方を同時に pin する。
    let desktop = platform_root().join("desktop");
    assert!(
        desktop.is_dir(),
        "desktop family frame ディレクトリが存在しなければならない"
    );

    // windowing leaf 着手（ADR-0118）: desktop は Cargo.toml と src を持つ実 crate になった。
    let manifest_path = desktop.join("Cargo.toml");
    assert!(
        manifest_path.exists(),
        "desktop は ADR-0118 で windowing leaf に着手した実 crate（Cargo.toml を持つ）"
    );
    assert!(
        desktop.join("src").exists(),
        "windowing leaf の実装ソース（src）が存在する"
    );

    // crate は windowing Platform Front であって capability facade ではない: パッケージ名は
    // `hayate-platform-desktop`、native window を開く `hayate-desktop` bin を持つ。
    let manifest = fs::read_to_string(&manifest_path).expect("desktop/Cargo.toml を読める");
    assert!(
        manifest.contains("name = \"hayate-platform-desktop\""),
        "desktop crate は windowing Platform Front `hayate-platform-desktop`"
    );
    assert!(
        manifest.contains("hayate-desktop"),
        "desktop crate は native window を開く `hayate-desktop` bin を持つ（windowing leaf）"
    );

    // 芯の規律の構造的 pin: capability の契約（trait）は Core 所有なので、desktop crate 側は
    // capability trait（facade）を **一切定義しない**。src のどのソースにも `trait` 宣言が無いことで
    // 「空 facade / capability trait を adapter 側に切らない」を固定する（ADR-0117）。
    let mut trait_decls = Vec::new();
    for entry in fs::read_dir(desktop.join("src")).expect("desktop/src を列挙できる") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = fs::read_to_string(&path).expect("desktop src file を読める");
        for line in src.lines() {
            let t = line.trim_start();
            // doc/コメント行を除いた実コードの trait 宣言だけを拾う。
            if !t.starts_with("//") && (t.starts_with("trait ") || t.starts_with("pub trait ")) {
                trait_decls.push(format!("{}: {}", path.display(), t));
            }
        }
    }
    assert!(
        trait_decls.is_empty(),
        "desktop crate は capability trait/facade を定義しない（契約の正本は Core・ADR-0117）。\
         見つかった trait 宣言: {trait_decls:?}"
    );

    // doctrine の文言も README で pin する: windowing leaf に着手したこと（ADR-0118）と、
    // capability facade は依然 0・契約正本は Core であること。
    let readme = fs::read_to_string(desktop.join("README.md"))
        .expect("desktop/README.md が doctrine を述べる");
    assert!(
        readme.contains("windowing leaf") && readme.contains("ADR-0118"),
        "desktop README は ADR-0118 で windowing leaf に着手したことを明記する"
    );
    assert!(
        readme.contains("capability facade") && readme.contains("0"),
        "desktop README は capability facade が依然 0 であることを明記する"
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
    for capability in [
        "audio",
        "clipboard",
        "notification",
        "haptics",
        "file picker",
    ] {
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
