//! Miharashi の Android ホストが App Bundle を取得する源（#532）。
//!
//! ADR-0112 の `app_tsubame` は当初、ビルド時に APK へ焼いた `assets/tsubame.js` を
//! 読んで Hermes に eval していた。Miharashi（見晴らし）はホストを再ビルドせずバンドルだけ
//! 差し替える dev-client なので、源を**実行時ネットワーク fetch** に替える（Miharashi
//! CONTEXT.md / #528 Web ホストと対称）。eval シームは不変 — 取得した JS ソース String を
//! そのまま `new_hermes_app(.., &bundle)` に渡す。
//!
//! HTTP は素の TCP 上の最小 HTTP/1.1 GET で張る（依存追加なし）。これにより、源の難所
//! ——リクエスト組み立てと応答 marshalling——を**ホストで読めるピュアなシーム**に切り出せる
//! （`build_bundle_request` / `parse_bundle_response`）。実 socket I/O（`fetch_bundle`）は
//! その上の薄いグルーで、実描画はローカル実機で検証する（本 issue 外）。

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::dev_server_target::DevServerTarget;

/// dev-server がバンドルを配信する HTTP ルート。`@miharashi/dev-server` の `BUNDLE_ROUTE`
/// と一致させる wire 契約（node 依存をネイティブへ持ち込まないため値で複製する）。
const BUNDLE_ROUTE: &str = "/bundle.js";
/// バンドル fetch のタイムアウト。応答しない dev-server で永久に待たない上限
/// （#528 Web ホストの `BUNDLE_FETCH_TIMEOUT_MS = 10_000` と対称）。
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP ヘッダブロックと本文を分かつ終端（空行）。
const HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";
/// HTTP 行の区切り。
const CRLF: &[u8] = b"\r\n";
/// バンドル配信成功の status code。これ以外は明示エラーにする（#528 Web ホストの `res.ok` と対称）。
const HTTP_OK: u16 = 200;

/// 応答の marshalling / 取得失敗。
#[derive(Debug, PartialEq, Eq)]
pub enum BundleFetchError {
    /// 応答が HTTP/1.1 として解せない（status line / ヘッダ終端が無い）。
    MalformedResponse,
    /// dev-server が 200 以外を返した（例: バンドル未配信で 404）。
    HttpStatus(u16),
    /// dev-server のアドレスを解決できなかった。
    AddressResolution,
    /// 接続 / 読み書きの socket I/O 失敗。
    Io(std::io::ErrorKind),
}

/// 解決済み [`DevServerTarget`]（端末 UI が入れた URL、無ければ既定）からバンドルを取得する。
/// `app_tsubame::run` はこれを APK asset 読み込みの代わりに呼び、得た JS ソースをそのまま
/// `new_hermes_app(.., &bundle)` の eval シームへ渡す。route / timeout は名前付き定数、host / port は
/// target が持つ（同じ target が reload 購読も駆動する＝保持・#534）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn fetch_from(target: &DevServerTarget) -> Result<String, BundleFetchError> {
    fetch_bundle(target.host(), target.port(), BUNDLE_ROUTE, FETCH_TIMEOUT)
}

/// dev-server URL（host:port）からバンドルルートを GET し、JS ソース String を取得する。
///
/// 素の TCP 上に最小 HTTP/1.1 を張る薄いグルー。難所（リクエスト組み立て・応答 marshalling）は
/// `build_bundle_request` / `parse_bundle_response` の純粋シームが持ち、ここは socket I/O だけを
/// 担う。`timeout` を connect / read / write に一様に課し、応答しない dev-server で永久に
/// 待たない（#528 Web ホストの `AbortSignal.timeout` と対称）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn fetch_bundle(
    host: &str,
    port: u16,
    path: &str,
    timeout: Duration,
) -> Result<String, BundleFetchError> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| BundleFetchError::Io(e.kind()))?
        .next()
        .ok_or(BundleFetchError::AddressResolution)?;

    let mut stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| BundleFetchError::Io(e.kind()))?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|e| BundleFetchError::Io(e.kind()))?;

    let request = build_bundle_request(host, port, path);
    stream
        .write_all(request.as_bytes())
        .map_err(|e| BundleFetchError::Io(e.kind()))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| BundleFetchError::Io(e.kind()))?;

    parse_bundle_response(&raw)
}

/// dev-server に投げる最小 HTTP/1.1 GET リクエストを組み立てる。
///
/// `Connection: close` を付け、keep-alive な dev-server（Node http の既定）に応答後 socket を
/// 閉じさせる。これで `fetch_bundle` 側は Content-Length を解さずとも read-to-EOF で本文末まで
/// 読める。`Host` ヘッダは host:port を載せる（HTTP/1.1 必須）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn build_bundle_request(host: &str, port: u16, path: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\nAccept: */*\r\n\r\n"
    )
}

/// 取得した生の HTTP 応答バイト列から JS ソース本文を取り出す。
///
/// `HTTP/1.1 <status> ...\r\n<headers>\r\n\r\n<body>` を head/body に割り、status 200 の
/// ときだけ body を JS ソース String（lossy UTF-8）として返す。
pub fn parse_bundle_response(raw: &[u8]) -> Result<String, BundleFetchError> {
    let split = raw
        .windows(HEADER_TERMINATOR.len())
        .position(|w| w == HEADER_TERMINATOR)
        .ok_or(BundleFetchError::MalformedResponse)?;
    let head = &raw[..split];
    let body = &raw[split + HEADER_TERMINATOR.len()..];

    let status = parse_status_code(head)?;
    if status != HTTP_OK {
        return Err(BundleFetchError::HttpStatus(status));
    }
    Ok(String::from_utf8_lossy(body).into_owned())
}

/// status line（`HTTP/1.1 <code> <reason>`）から数値 status code を取り出す。
fn parse_status_code(head: &[u8]) -> Result<u16, BundleFetchError> {
    let line_end = head
        .windows(CRLF.len())
        .position(|w| w == CRLF)
        .unwrap_or(head.len());
    let status_line =
        std::str::from_utf8(&head[..line_end]).map_err(|_| BundleFetchError::MalformedResponse)?;
    status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or(BundleFetchError::MalformedResponse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    /// 1 接続だけ受け、リクエストを読み捨てて与えた応答を書き戻し socket を閉じる最小 HTTP サーバ。
    /// 実 dev-server の代役として `fetch_bundle` の全経路（request → TCP → parse）をホストで貫く。
    fn serve_once(response: &'static [u8]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            stream.write_all(response).unwrap();
            // drop(stream) で socket が閉じ、クライアント側の read-to-EOF が終端する。
        });
        port
    }

    #[test]
    fn extracts_js_source_from_a_200_response() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-type: application/javascript\r\n\r\nglobalThis.__tsubame = {};";
        assert_eq!(
            parse_bundle_response(raw).unwrap(),
            "globalThis.__tsubame = {};"
        );
    }

    #[test]
    fn tunables_are_named_constants_matching_the_dev_server_contract() {
        // dev-server の BUNDLE_ROUTE（`@miharashi/dev-server`）と一致する wire 契約。
        assert_eq!(BUNDLE_ROUTE, "/bundle.js");
        // #528 Web ホストの BUNDLE_FETCH_TIMEOUT_MS = 10_000 と対称。
        assert_eq!(FETCH_TIMEOUT, Duration::from_secs(10));
        // host / port は端末 UI が入れた URL から解決する DevServerTarget が持つ（既定は dev_server_target、#534）。
    }

    #[test]
    fn fetches_from_the_resolved_target_host_and_port() {
        // 端末 UI が入れた URL を解決した target の host:port で取りに行く（同じ target が reload も駆動する）。
        let port = serve_once(
            b"HTTP/1.1 200 OK\r\ncontent-type: application/javascript\r\n\r\nglobalThis.__entered = 1;",
        );
        let target = crate::dev_server_target::resolve(Some(&format!("127.0.0.1:{port}")));
        assert_eq!(fetch_from(&target).unwrap(), "globalThis.__entered = 1;");
    }

    #[test]
    fn fetch_bundle_retrieves_js_source_over_tcp() {
        let port = serve_once(
            b"HTTP/1.1 200 OK\r\ncontent-type: application/javascript\r\n\r\nglobalThis.__miharashiMount = () => {};",
        );
        let source =
            fetch_bundle("127.0.0.1", port, "/bundle.js", Duration::from_secs(5)).unwrap();
        assert_eq!(source, "globalThis.__miharashiMount = () => {};");
    }

    #[test]
    fn request_is_a_get_for_the_route_that_closes_the_connection() {
        let req = build_bundle_request("10.0.2.2", 5179, "/bundle.js");
        // dev-server が受け取る wire 上の最初の行。
        assert!(req.starts_with("GET /bundle.js HTTP/1.1\r\n"), "got: {req:?}");
        // 仮想ホスト解決のための Host ヘッダ（host:port）。
        assert!(req.contains("Host: 10.0.2.2:5179\r\n"), "got: {req:?}");
        // keep-alive な dev-server に応答後 socket を閉じさせ、read-to-EOF を終端させる。
        assert!(req.contains("Connection: close\r\n"), "got: {req:?}");
        // ヘッダブロックは空行で終端する。
        assert!(req.ends_with("\r\n\r\n"), "got: {req:?}");
    }

    #[test]
    fn rejects_non_200_status() {
        let raw = b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n";
        assert_eq!(
            parse_bundle_response(raw),
            Err(BundleFetchError::HttpStatus(404))
        );
    }
}
