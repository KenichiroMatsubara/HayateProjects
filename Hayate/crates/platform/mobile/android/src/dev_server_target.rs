//! Torimi の Android ホストが接続する dev-server / Demo Endpoint の指定（#534, #740）。
//!
//! #532/#533 までは接続先 dev-server が `bundle_source` のハードコード定数だった。#534 で
//! **端末上で入力**できるようにし、#740（ADR-0002 前半）で target を `host:port`（path 破棄・
//! scheme 破棄）から **scheme-aware かつ path 保持のフル URL** に広げた——複数デモをパスで
//! 区別する Demo Endpoint（ADR-0003）の前提。バンドル fetch（HTTP(S)）と reload 購読（WS）は
//! **同じ target で駆動**する（保持）。入力が無ければ既定 target（エミュレータ loopback）に落ちる。
//!
//! ここはプラットフォーム非依存のピュアなシーム（URL 正規化・既定・入力読み）なのでホストで
//! 契約テストする。実 UI（Kotlin の EditText）と実機 fetch/boot はローカル実機で検証する
//! （本 issue 外）。

use std::path::Path;

/// 端末 UI（Kotlin の EditText）が入力した dev-server URL を書き込み、ネイティブが読み戻すファイル名。
/// アプリの internal data dir（`AndroidApp::internal_data_path`）直下に置く、Kotlin（writer）↔ Rust
/// （reader）間の wire 契約。再起動・reload を跨いで読み戻すので入力値が保持される（保持・再接続）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) const DEV_SERVER_URL_FILE: &str = "torimi-dev-server-url.txt";

/// 既定の dev-server ホスト。Android エミュレータからホスト機の loopback へ抜ける別名アドレス。
/// 端末 UI で URL を入れなかったときの落とし先（#532 のハードコード値を target の既定として引き継ぐ）。
pub(crate) const DEFAULT_DEV_SERVER_HOST: &str = "10.0.2.2";
/// 既定の dev-server ポート（#528 Web ホストの docstring 例 `http://127.0.0.1:5179` と同値）。
/// `http://` の scheme 既定ポートもこれ（LAN dev の従来入力の意味を変えない・ADR-0002）。
pub(crate) const DEFAULT_DEV_SERVER_PORT: u16 = 5179;
/// `https://` の scheme 既定ポート。公開 Demo Endpoint（ADR-0003）の URL はポート無しで
/// 貼られるので、標準の 443 に広げる。
pub(crate) const HTTPS_DEFAULT_PORT: u16 = 443;

/// release ビルドの既定接続先＝公開 Demo Endpoint（ADR-0003）の URL。テスター・審査者が
/// ゼロ入力でデモに到達するための落とし先で、初回起動はここから Demo Manifest を取って先頭
/// デモを自動ロードする（#743）。**ビルド構成で差し替え可能** — 実際の workers.dev サブドメイン
/// （account 依存）は別 account でビルドするときビルド時 `TORIMI_DEMO_ENDPOINT_URL` で上書きする
/// （[`release_demo_endpoint_url`]）。既定値は Worker 名（`torimi-demo-endpoint`・wrangler.jsonc）と
/// この repo が配信する account の workers.dev サブドメイン（`pinara`）由来。
pub const DEFAULT_DEMO_ENDPOINT_URL: &str = "https://torimi-demo-endpoint.pinara.workers.dev";
/// path 無し入力の正規化先。「バンドルルートは既定の wire 契約に任せる」の意（`bundle_source`
/// が既定ルートへ広げる）。
const ROOT_PATH: &str = "/";

/// target の scheme。cleartext http は LAN dev 用途、公開配信は https（ADR-0002）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
}

impl Scheme {
    /// URL に載せる scheme 文字列。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn as_str(self) -> &'static str {
        match self {
            Scheme::Http => "http",
            Scheme::Https => "https",
        }
    }

    /// ポートを省いた入力に使う scheme 既定ポート（https → 443 / http → dev-server 既定）。
    fn default_port(self) -> u16 {
        match self {
            Scheme::Http => DEFAULT_DEV_SERVER_PORT,
            Scheme::Https => HTTPS_DEFAULT_PORT,
        }
    }
}

/// 端末 UI が入力した URL から正規化した接続先（scheme-aware・path 保持のフル URL・ADR-0002）。
/// バンドル fetch（HTTP(S)）と reload 購読（WS）の単一の source of truth で、両方をこの target で
/// 駆動する（保持）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerTarget {
    scheme: Scheme,
    host: String,
    port: u16,
    path: String,
}

impl DevServerTarget {
    /// バンドル fetch の scheme（http = LAN dev / https = 公開 Demo Endpoint）。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }
    /// バンドル fetch / reload WS が接続するホスト。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn host(&self) -> &str {
        &self.host
    }
    /// 接続ポート（明示が無ければ scheme 既定）。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn port(&self) -> u16 {
        self.port
    }
    /// 入力 URL の path（保持）。`/` は「path 指定なし＝既定バンドルルート」の意。
    /// 複数デモをパスで区別する Demo Endpoint（ADR-0003）の前提。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Default for DevServerTarget {
    /// 端末 UI で URL を入れなかったときの落とし先（エミュレータ loopback）。
    fn default() -> Self {
        DevServerTarget {
            scheme: Scheme::Http,
            host: DEFAULT_DEV_SERVER_HOST.to_owned(),
            port: DEFAULT_DEV_SERVER_PORT,
            path: ROOT_PATH.to_owned(),
        }
    }
}

/// 端末 UI が入力した URL 文字列を [`DevServerTarget`] に正規化する。空 / host 欠落 / ポート不正 /
/// 未知 scheme など使えない入力は `None`（呼び出し側 [`resolve`] が既定 target に落とす）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn parse(input: &str) -> Option<DevServerTarget> {
    let (scheme, rest) = split_scheme(input.trim())?;
    let (authority, path) = split_path(rest);
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, scheme.default_port()),
    };
    if host.is_empty() {
        return None;
    }
    Some(DevServerTarget {
        scheme,
        host: host.to_owned(),
        port,
        path: path.to_owned(),
    })
}

/// `scheme://` 前置を scheme と authority 以降に分ける。scheme 無しは従来の LAN dev 入力
/// （`host:port`）として http。reload WS の URL を貼っても同じ target に解決できるよう
/// ws/wss は対応する http/https に写す。未知 scheme は `None`（既定 target への fallback へ）。
fn split_scheme(input: &str) -> Option<(Scheme, &str)> {
    match input.find("://") {
        None => Some((Scheme::Http, input)),
        Some(i) => {
            let scheme = match &input[..i] {
                "http" | "ws" => Scheme::Http,
                "https" | "wss" => Scheme::Https,
                _ => return None,
            };
            Some((scheme, &input[i + "://".len()..]))
        }
    }
}

/// authority（`host[:port]`）と path（最初の `/` から先）に分ける。path は**保持**する
/// （ADR-0002 / ADR-0003：複数デモをパスで区別する）。無し・素の `/` は [`ROOT_PATH`] に正規化。
fn split_path(rest: &str) -> (&str, &str) {
    match rest.find('/') {
        Some(i) if rest.len() > i + 1 => (&rest[..i], &rest[i..]),
        Some(i) => (&rest[..i], ROOT_PATH),
        None => (rest, ROOT_PATH),
    }
}

/// release ビルドが差し込む Demo Endpoint URL。既定は [`DEFAULT_DEMO_ENDPOINT_URL`] だが、
/// **ビルド構成で差し替え可能** にするためコンパイル時 env `TORIMI_DEMO_ENDPOINT_URL` があれば
/// それを使う（account 依存の実サブドメインを AAB ビルドで注入する・ADR-0003 / #743）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn release_demo_endpoint_url() -> &'static str {
    option_env!("TORIMI_DEMO_ENDPOINT_URL").unwrap_or(DEFAULT_DEMO_ENDPOINT_URL)
}

/// release ビルドの既定 target＝公開 Demo Endpoint（ADR-0003）。ここを起点に Demo Manifest を
/// 取り先頭デモを自動ロードする（#743）。URL が壊れていてもホストを殺さないよう、解釈不能なら
/// エミュレータ loopback（[`DevServerTarget::default`]）に落ちる。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn release_default_target() -> DevServerTarget {
    parse(release_demo_endpoint_url()).unwrap_or_default()
}

/// ビルド既定の接続先。**release は公開 Demo Endpoint、debug はエミュレータ loopback** に分ける
/// （ADR-0003 / #743）。debug 既定（[`DevServerTarget::default`]）は #534 のまま不変で、LAN dev の
/// 従来経路（URL 入力／QR）はこの分岐に一切影響されない。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn build_default_target() -> DevServerTarget {
    if cfg!(debug_assertions) {
        DevServerTarget::default()
    } else {
        release_default_target()
    }
}

/// 端末 UI が入力した URL（無ければ `None`）を target に解決する。入力が無ければ**ビルド既定** target
/// に落とす（release=Demo Endpoint / debug=loopback・#743）。バンドル fetch と reload 購読はどちらも
/// この解決済み target を使う（単一の source of truth = 保持）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn resolve(entered: Option<&str>) -> DevServerTarget {
    entered.and_then(parse).unwrap_or_else(build_default_target)
}

/// 端末 UI が internal data dir に書いた dev-server URL を読み戻す。data dir が無い（`None`）/ ファイル
/// 未作成 / 空ならば `None`。毎 boot・reload で読み直すので、入力値は再起動・再接続を跨いで効く（保持）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn read_entered_url(internal_data_path: Option<&Path>) -> Option<String> {
    let contents = std::fs::read_to_string(internal_data_path?.join(DEV_SERVER_URL_FILE)).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// 端末 UI が書いた URL を読み戻して target に解決する、ホストの単一の入口。`app_tsubame::run` は
/// これ 1 つを呼び、得た target でバンドル fetch（HTTP）と reload 購読（WS）の両方を駆動する（保持）。
/// data dir が無い / 未入力 / 不正なら既定 target に落ちる。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn resolve_entered(internal_data_path: Option<&Path>) -> DevServerTarget {
    resolve(read_entered_url(internal_data_path).as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_host_and_port_as_cleartext_http() {
        // scheme を書かない LAN dev 入力（従来形式）は http のまま扱う（cleartext は LAN dev 用途・ADR-0002）。
        let target = parse("10.0.2.2:5179").unwrap();
        assert_eq!(target.scheme(), Scheme::Http);
        assert_eq!(target.host(), "10.0.2.2");
        assert_eq!(target.port(), 5179);
        assert_eq!(target.path(), "/");
    }

    #[test]
    fn a_pasted_http_origin_keeps_its_scheme_host_and_port() {
        let target = parse("http://192.168.1.5:8080").unwrap();
        assert_eq!(target.scheme(), Scheme::Http);
        assert_eq!(target.host(), "192.168.1.5");
        assert_eq!(target.port(), 8080);
    }

    #[test]
    fn https_defaults_to_port_443() {
        // 公開 Demo Endpoint（ADR-0003）の URL はポート無しで貼られる。scheme 既定ポートに広げる（ADR-0002）。
        let target = parse("https://torimi-demo.example.workers.dev").unwrap();
        assert_eq!(target.scheme(), Scheme::Https);
        assert_eq!(target.host(), "torimi-demo.example.workers.dev");
        assert_eq!(target.port(), HTTPS_DEFAULT_PORT);
        assert_eq!(target.port(), 443);
    }

    #[test]
    fn http_without_a_port_defaults_to_the_dev_server_port() {
        // http の既定は dev-server の既定ポート（5179）。従来の bare-host 入力の意味を変えない。
        let target = parse("http://192.168.1.5").unwrap();
        assert_eq!(target.port(), DEFAULT_DEV_SERVER_PORT);
        assert_eq!(target.port(), 5179);
    }

    #[test]
    fn preserves_the_url_path_so_demos_can_be_told_apart() {
        // 複数デモをパスで区別する Demo Endpoint（ADR-0003）の前提：path は捨てず保持する
        // （従来の「host:port だけ取り path 破棄」を広げる・ADR-0002）。
        let target = parse("https://demo.example/solid/bundle.js").unwrap();
        assert_eq!(target.path(), "/solid/bundle.js");
        assert_eq!(target.port(), HTTPS_DEFAULT_PORT);

        let lan = parse("192.168.1.5:8080/react/bundle.js").unwrap();
        assert_eq!(lan.scheme(), Scheme::Http);
        assert_eq!(lan.path(), "/react/bundle.js");
        assert_eq!(lan.port(), 8080);
    }

    #[test]
    fn a_bare_or_trailing_slash_normalizes_to_the_root_path() {
        // path 無し / 素の `/` は「バンドルルートは既定の wire 契約に任せる」の意で root に正規化する。
        assert_eq!(parse("192.168.1.5:8080").unwrap().path(), "/");
        assert_eq!(parse("http://192.168.1.5:8080/").unwrap().path(), "/");
    }

    #[test]
    fn ws_schemes_map_onto_the_matching_http_scheme() {
        // reload の WS URL（ws://…/reload）を貼っても同じ target に解決できる（従来挙動の継承）。
        assert_eq!(
            parse("ws://192.168.1.5:5179").unwrap().scheme(),
            Scheme::Http
        );
        assert_eq!(parse("wss://demo.example").unwrap().scheme(), Scheme::Https);
        assert_eq!(
            parse("wss://demo.example").unwrap().port(),
            HTTPS_DEFAULT_PORT
        );
    }

    #[test]
    fn a_bare_host_uses_the_default_port() {
        // ポートを省いた入力（`192.168.1.5`）は既定ポートに揃える。
        let target = parse("192.168.1.5").unwrap();
        assert_eq!(target.host(), "192.168.1.5");
        assert_eq!(target.port(), DEFAULT_DEV_SERVER_PORT);
    }

    #[test]
    fn falls_back_to_the_named_default_when_nothing_is_entered() {
        // 端末 UI で URL を入れなかった経路。既定はエミュレータ loopback（#532 のハードコード値を継承）。
        let target = resolve(None);
        assert_eq!(target.scheme(), Scheme::Http);
        assert_eq!(target.host(), DEFAULT_DEV_SERVER_HOST);
        assert_eq!(target.port(), DEFAULT_DEV_SERVER_PORT);
        assert_eq!(target.host(), "10.0.2.2");
        assert_eq!(target.port(), 5179);
        assert_eq!(target.path(), "/");
    }

    #[test]
    fn the_release_default_is_the_public_demo_endpoint_as_a_named_constant() {
        // release 既定接続先は公開 Demo Endpoint（ADR-0003）で、名前付き定数（ビルド構成で差し替え可）。
        // 既定値は https の workers.dev サブドメインで、443 に正規化される（scheme 既定ポート・#740）。
        let url = release_demo_endpoint_url();
        assert!(
            url.starts_with("https://"),
            "release default must be an https Demo Endpoint URL: {url}"
        );
        let target = release_default_target();
        assert_eq!(target.scheme(), Scheme::Https);
        assert_eq!(target.port(), HTTPS_DEFAULT_PORT);
        // 既定定数（未上書き時）は Worker 名＋配信 account（pinara）の workers.dev サブドメイン。
        assert_eq!(
            DEFAULT_DEMO_ENDPOINT_URL,
            "https://torimi-demo-endpoint.pinara.workers.dev"
        );
    }

    #[test]
    fn the_debug_default_is_the_emulator_loopback_unchanged() {
        // debug 既定（エミュレータ loopback）は #534 のまま不変で、release の Demo Endpoint とは別物。
        // host の cargo test は debug ビルドなので、ビルド既定はこの loopback に一致する。
        assert!(cfg!(debug_assertions), "host tests run in debug");
        assert_eq!(build_default_target(), DevServerTarget::default());
        assert_eq!(build_default_target().host(), "10.0.2.2");
        // release 既定は明確に別物（loopback ではない）。
        assert_ne!(release_default_target(), DevServerTarget::default());
    }

    #[test]
    fn blank_or_malformed_input_falls_back_to_the_default() {
        // 不正な端末入力（空・空白のみ・host 欠落・ポート非数値・未知 scheme）はホストを
        // クラッシュさせず既定へ。
        for bad in [
            "",
            "   ",
            ":5179",
            "http://",
            "10.0.2.2:not-a-port",
            "ftp://x.example",
        ] {
            assert_eq!(parse(bad), None, "{bad:?} must not parse to a target");
            assert_eq!(
                resolve(Some(bad)),
                DevServerTarget::default(),
                "{bad:?} resolves to default"
            );
        }
    }

    /// テスト専用の使い捨て data dir（Kotlin が書く internal data dir の代役）。
    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("torimi-dst-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_back_the_url_the_device_ui_persisted() {
        // 端末 UI が書いた URL を data dir から読み戻す。改行込みでも trim される（EditText の末尾改行）。
        let dir = temp_data_dir("persisted");
        std::fs::write(dir.join(DEV_SERVER_URL_FILE), "192.168.1.5:8080\n").unwrap();

        // 再起動・再接続のたびに読み直しても同じ値（保持）。
        assert_eq!(
            read_entered_url(Some(&dir)).as_deref(),
            Some("192.168.1.5:8080")
        );
        assert_eq!(
            read_entered_url(Some(&dir)).as_deref(),
            Some("192.168.1.5:8080")
        );

        let target = resolve_entered(Some(&dir));
        assert_eq!(target.host(), "192.168.1.5");
        assert_eq!(target.port(), 8080);
    }

    #[test]
    fn resolves_to_the_default_when_no_url_was_persisted() {
        // data dir が無い、ファイル未作成、空ファイル — どれも既定 target に落ちる（クラッシュしない）。
        assert_eq!(read_entered_url(None), None);
        assert_eq!(resolve_entered(None), DevServerTarget::default());

        let empty_dir = temp_data_dir("empty");
        assert_eq!(read_entered_url(Some(&empty_dir)), None);
        assert_eq!(
            resolve_entered(Some(&empty_dir)),
            DevServerTarget::default()
        );

        let blank_dir = temp_data_dir("blank");
        std::fs::write(blank_dir.join(DEV_SERVER_URL_FILE), "  \n").unwrap();
        assert_eq!(read_entered_url(Some(&blank_dir)), None);
        assert_eq!(
            resolve_entered(Some(&blank_dir)),
            DevServerTarget::default()
        );
    }
}
