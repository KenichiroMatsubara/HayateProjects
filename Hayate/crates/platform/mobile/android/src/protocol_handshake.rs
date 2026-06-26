//! protocol version 突き合わせ（Miharashi Android ホスト, #533）。
//!
//! バンドル（encoder）が埋めた wire 定数バージョンと、ホスト（decoder）に焼き込まれた
//! バージョンを突き合わせ、一致時のみ mount を許す。不一致は両バージョンを含む明示エラーに
//! して謎クラッシュにしない。これは Web の `@miharashi/protocol-handshake`（#530）の純関数
//! `checkProtocolVersion` と**同じ contract**（同じ突き合わせ規則・同じメッセージ・同じ wire
//! global 名）を Rust に写したもの。両ホストは同じ source of truth
//! （`@hayate/protocol-spec` の manifest version＝`hayate_core::wire::PROTOCOL_VERSION`）を
//! 焼き込み版数に使う（CONTEXT.md「Protocol Version」/ ADR-0001）。
//!
//! TS パッケージは Rust から直接は使えないため、wire 契約（global 名）は値で複製する
//! （`bundle_source` の `BUNDLE_ROUTE` 複製と同じ方針）。プラットフォーム非依存の純 Rust
//! なのでホストで `cargo test` できる。

/// eval 済みバンドルが自身の wire 定数バージョンを露出する JS global プロパティ名。Web の
/// `MIHARASHI_PROTOCOL_VERSION_GLOBAL`（`@miharashi/protocol-handshake`）と一致させる wire 契約。
/// mount を渡す `__miharashiMount` と対称の、バンドル → ホストの受け渡しシーム。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub const MIHARASHI_PROTOCOL_VERSION_GLOBAL: &str = "__miharashiProtocolVersion";

/// protocol version 不一致。ホスト/バンドル両方の版数と、表示用の明示メッセージを構造化して運ぶ。
/// 合成ルート（Android ホスト）はこれを使い、mount もクラッシュもさせず明示エラーを出す（#530）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolMismatch {
    /// このホスト（decoder）に焼き込まれた版数。
    pub host_version: u32,
    /// バンドル（encoder）が埋めた版数。未埋め込み（契約違反）なら `None`。
    pub bundle_version: Option<u32>,
    /// 表示用の明示メッセージ（ホスト/バンドル両版数を含む）。
    pub message: String,
}

/// バンドル（encoder）が埋めた wire 定数バージョンと、ホスト（decoder）に焼き込まれた版数を
/// 突き合わせる。一致なら `Ok(())`（mount を許す）、不一致は両版数を含む [`ProtocolMismatch`] を
/// `Err` で返す（謎クラッシュにしない）。Web の `checkProtocolVersion`（#530）と同じ規則・
/// 同じメッセージ。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn check_protocol_version(
    host_version: u32,
    bundle_version: Option<u32>,
) -> Result<(), ProtocolMismatch> {
    if bundle_version == Some(host_version) {
        return Ok(());
    }
    let bundle_label = match bundle_version {
        Some(v) => format!("v{v}"),
        None => "version 未埋め込み".to_owned(),
    };
    Err(ProtocolMismatch {
        host_version,
        bundle_version,
        message: format!("このホストは protocol v{host_version}、バンドルは {bundle_label}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Web の protocol-handshake.test.ts（#530）と対称の単体契約テスト。

    #[test]
    fn matching_versions_allow_mount() {
        assert_eq!(check_protocol_version(1, Some(1)), Ok(()));
    }

    #[test]
    fn mismatch_reports_both_versions() {
        let mismatch = check_protocol_version(1, Some(2)).unwrap_err();
        assert_eq!(mismatch.host_version, 1);
        assert_eq!(mismatch.bundle_version, Some(2));
    }

    #[test]
    fn mismatch_message_names_both_host_and_bundle_versions() {
        let mismatch = check_protocol_version(3, Some(7)).unwrap_err();
        assert!(mismatch.message.contains('3'), "got: {}", mismatch.message);
        assert!(mismatch.message.contains('7'), "got: {}", mismatch.message);
    }

    #[test]
    fn missing_bundle_version_is_an_explicit_mismatch() {
        // バンドルが version 未埋め込み（契約違反）でも mount に通さず、明示エラーにする。
        let mismatch = check_protocol_version(1, None).unwrap_err();
        assert_eq!(mismatch.host_version, 1);
        assert_eq!(mismatch.bundle_version, None);
        assert!(mismatch.message.contains('1'), "got: {}", mismatch.message);
    }

    #[test]
    fn protocol_version_global_matches_the_web_wire_contract() {
        // Web の MIHARASHI_PROTOCOL_VERSION_GLOBAL（@miharashi/protocol-handshake）と同値の wire 契約。
        assert_eq!(MIHARASHI_PROTOCOL_VERSION_GLOBAL, "__miharashiProtocolVersion");
    }

    #[test]
    fn host_version_source_of_truth_is_the_native_decoder_version() {
        // ホストの焼き込み版数は Web と同じ source of truth（manifest version）＝ネイティブ decoder の
        // `hayate_core::wire::PROTOCOL_VERSION`。突き合わせがその版数で通ることを固定する（#530 共有）。
        let host_version = hayate_core::wire::PROTOCOL_VERSION;
        assert_eq!(check_protocol_version(host_version, Some(host_version)), Ok(()));
    }
}
