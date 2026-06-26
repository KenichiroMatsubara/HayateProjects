//! Miharashi の Android ホストが接続する dev-server の指定（#534）。
//!
//! #532/#533 までは接続先 dev-server が `bundle_source` のハードコード定数だった。本 issue は
//! それを**端末上で入力**できるようにする：端末 UI が入れた URL 文字列を `host:port` へ正規化し、
//! バンドル fetch（HTTP）と reload 購読（WS）の**両方を同じ target で駆動**する（保持）。入力が
//! 無ければ既定 target（エミュレータ loopback）に落ちる。
//!
//! ここはプラットフォーム非依存のピュアなシーム（パース・既定・入力読み）なのでホストで契約テスト
//! する。実 UI（Kotlin の EditText）と実機 fetch/boot はローカル実機で検証する（本 issue 外）。
//! QR 読み取りは将来スコープ（本 issue は URL 直指定まで）。

use std::path::Path;

/// 端末 UI（Kotlin の EditText）が入力した dev-server URL を書き込み、ネイティブが読み戻すファイル名。
/// アプリの internal data dir（`AndroidApp::internal_data_path`）直下に置く、Kotlin（writer）↔ Rust
/// （reader）間の wire 契約。再起動・reload を跨いで読み戻すので入力値が保持される（保持・再接続）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) const DEV_SERVER_URL_FILE: &str = "miharashi-dev-server-url.txt";

/// 既定の dev-server ホスト。Android エミュレータからホスト機の loopback へ抜ける別名アドレス。
/// 端末 UI で URL を入れなかったときの落とし先（#532 のハードコード値を target の既定として引き継ぐ）。
pub(crate) const DEFAULT_DEV_SERVER_HOST: &str = "10.0.2.2";
/// 既定の dev-server ポート（#528 Web ホストの docstring 例 `http://127.0.0.1:5179` と同値）。
pub(crate) const DEFAULT_DEV_SERVER_PORT: u16 = 5179;

/// 端末 UI が入力した dev-server URL から正規化した接続先。バンドル fetch（HTTP）と reload 購読（WS）の
/// 単一の source of truth で、両方をこの host/port で駆動する（保持）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerTarget {
    host: String,
    port: u16,
}

impl DevServerTarget {
    /// バンドル fetch / reload WS が接続する dev-server ホスト。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn host(&self) -> &str {
        &self.host
    }
    /// dev-server ポート。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Default for DevServerTarget {
    /// 端末 UI で URL を入れなかったときの落とし先（エミュレータ loopback）。
    fn default() -> Self {
        DevServerTarget {
            host: DEFAULT_DEV_SERVER_HOST.to_owned(),
            port: DEFAULT_DEV_SERVER_PORT,
        }
    }
}

/// 端末 UI が入力した dev-server URL 文字列を [`DevServerTarget`] に正規化する。空 / host 欠落 /
/// ポート不正など使えない入力は `None`（呼び出し側 [`resolve`] が既定 target に落とす）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn parse(input: &str) -> Option<DevServerTarget> {
    let authority = strip_path(strip_scheme(input.trim()));
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, DEFAULT_DEV_SERVER_PORT),
    };
    if host.is_empty() {
        return None;
    }
    Some(DevServerTarget {
        host: host.to_owned(),
        port,
    })
}

/// `scheme://` 前置（http/https/ws/wss）を落として authority 以降を返す。端末で origin を貼っても
/// host:port が取れるようにする（Web ホストの `new URL` 相当を依存追加なしで）。
fn strip_scheme(input: &str) -> &str {
    match input.find("://") {
        Some(i) => &input[i + "://".len()..],
        None => input,
    }
}

/// authority（`host[:port]`）以降の path（最初の `/` から先）を落とす。バンドル / reload ルートは
/// 固定の wire 契約なので、ユーザが入れた path 部分は採らない。
fn strip_path(authority: &str) -> &str {
    match authority.find('/') {
        Some(i) => &authority[..i],
        None => authority,
    }
}

/// 端末 UI が入力した URL（無ければ `None`）を target に解決する。入力が無ければ既定 target に落とす。
/// バンドル fetch と reload 購読はどちらもこの解決済み target を使う（単一の source of truth = 保持）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn resolve(entered: Option<&str>) -> DevServerTarget {
    entered.and_then(parse).unwrap_or_default()
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
    fn parses_a_plain_host_and_port() {
        let target = parse("10.0.2.2:5179").unwrap();
        assert_eq!(target.host(), "10.0.2.2");
        assert_eq!(target.port(), 5179);
    }

    #[test]
    fn strips_a_url_scheme_so_a_pasted_origin_works() {
        // 端末で `http://192.168.1.5:8080` を貼っても host:port が取れる（Web ホストの new URL と対称）。
        let target = parse("http://192.168.1.5:8080").unwrap();
        assert_eq!(target.host(), "192.168.1.5");
        assert_eq!(target.port(), 8080);
    }

    #[test]
    fn drops_a_trailing_path_keeping_only_host_and_port() {
        // バンドル / reload ルートは固定の wire 契約。ユーザが入れた path 部分は捨て、host:port だけ取る。
        assert_eq!(parse("http://192.168.1.5:8080/").unwrap().host(), "192.168.1.5");
        let with_route = parse("192.168.1.5:8080/bundle.js").unwrap();
        assert_eq!(with_route.host(), "192.168.1.5");
        assert_eq!(with_route.port(), 8080);
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
        assert_eq!(target.host(), DEFAULT_DEV_SERVER_HOST);
        assert_eq!(target.port(), DEFAULT_DEV_SERVER_PORT);
        assert_eq!(target.host(), "10.0.2.2");
        assert_eq!(target.port(), 5179);
    }

    #[test]
    fn blank_or_malformed_input_falls_back_to_the_default() {
        // 不正な端末入力（空・空白のみ・host 欠落・ポート非数値）はホストをクラッシュさせず既定へ。
        for bad in ["", "   ", ":5179", "http://", "10.0.2.2:not-a-port"] {
            assert_eq!(parse(bad), None, "{bad:?} must not parse to a target");
            assert_eq!(resolve(Some(bad)), DevServerTarget::default(), "{bad:?} resolves to default");
        }
    }

    /// テスト専用の使い捨て data dir（Kotlin が書く internal data dir の代役）。
    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("miharashi-dst-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_back_the_url_the_device_ui_persisted() {
        // 端末 UI が書いた URL を data dir から読み戻す。改行込みでも trim される（EditText の末尾改行）。
        let dir = temp_data_dir("persisted");
        std::fs::write(dir.join(DEV_SERVER_URL_FILE), "192.168.1.5:8080\n").unwrap();

        // 再起動・再接続のたびに読み直しても同じ値（保持）。
        assert_eq!(read_entered_url(Some(&dir)).as_deref(), Some("192.168.1.5:8080"));
        assert_eq!(read_entered_url(Some(&dir)).as_deref(), Some("192.168.1.5:8080"));

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
        assert_eq!(resolve_entered(Some(&empty_dir)), DevServerTarget::default());

        let blank_dir = temp_data_dir("blank");
        std::fs::write(blank_dir.join(DEV_SERVER_URL_FILE), "  \n").unwrap();
        assert_eq!(read_entered_url(Some(&blank_dir)), None);
        assert_eq!(resolve_entered(Some(&blank_dir)), DevServerTarget::default());
    }
}
