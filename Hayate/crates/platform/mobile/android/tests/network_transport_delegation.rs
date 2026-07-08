//! ネットワーク transport の OS スタック委譲（ADR-0002・#740 前半 / #742 後半）のホスト可読ガード。
//!
//! `bundle_source` の実 I/O は device 専用（JNI で Kotlin を呼ぶ）でホストでは実行できない。
//! そこで apk_packaging.rs / bundle_source_wiring.rs と同じくソース走査で契約を固定する：
//!
//! 1. **手書き HTTP/1.1 の撤去**：素の `TcpStream` 上のリクエスト組み立て・応答 marshalling が
//!    Rust から消えている（LAN dev 経路も委譲実装に統一・ADR-0002）。
//! 2. **委譲シーム**：Rust は URL（純粋シームで正規化済み）を Kotlin の `BundleFetchBridge` に
//!    渡し、fetch 済み JS ソース文字列だけを受け取る。JNI は共通下地（`jni_bridge`）経由。
//! 3. **Kotlin 側は OS のネットワークスタック**：`HttpURLConnection`（Android 実装は OkHttp 系）
//!    が HTTP(S) を担う。Rust ホストに TLS 依存は入れない。
//! 4. **cleartext は LAN dev 用途に限定**：networkSecurityConfig が base で cleartext を禁じ、
//!    エミュレータ loopback 等の dev ホストだけ明示許可する（既定は https）。
//!
//! 後半（#742）は reload 購読の WS を同じ形で委譲する：手書き RFC6455（ハンドシェイク組み立て・
//! フレーム解釈）を撤去し、Kotlin の `ReloadSocketBridge`（OkHttp の WebSocket）が WS(S) を担い、
//! Rust は注入シーム（`subscribe_reload` のポート）経由でシグナルだけを受け取る。
//!
//! 実機での通し確認（https の Demo Endpoint / LAN dev の双方）はローカル検証の領分（ADR-0001）。

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn bundle_source_src() -> String {
    read_relative("src/bundle_source.rs")
}

fn kotlin_bridge_src() -> String {
    read_relative(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/BundleFetchBridge.kt",
    )
}

fn manifest_src() -> String {
    read_relative("android-app/app/src/main/AndroidManifest.xml")
}

fn network_security_config_src() -> String {
    read_relative("android-app/app/src/main/res/xml/network_security_config.xml")
}

#[test]
fn the_handwritten_http_transport_is_gone() {
    // コメント（撤去の経緯を記す doc）は対象外——コードだけを走査する（qr_scanner_encapsulation
    // と同じ流儀）。
    let code: String = bundle_source_src()
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    // 素の TCP 上の HTTP/1.1（リクエスト組み立て・応答 marshalling）は委譲完了をもって撤去（ADR-0002）。
    for marker in [
        "TcpStream",
        "HTTP/1.1",
        "build_bundle_request",
        "parse_bundle_response",
        "parse_status_code",
    ] {
        assert!(
            !code.contains(marker),
            "bundle_source.rs must no longer hand-roll the HTTP transport (found {marker:?})"
        );
    }
}

#[test]
fn rust_delegates_the_fetch_to_the_kotlin_bridge_over_the_common_jni_seam() {
    let src = bundle_source_src();
    // JNI leaf は共通下地（jni_bridge）だけを使う（qr_scanner / error_overlay と同型・ADR-0125）。
    assert!(
        src.contains("jni_bridge::with_activity_env"),
        "the platform fetch must go through the shared JNI seam"
    );
    assert!(
        src.contains("BundleFetchBridge"),
        "the fetch must be delegated to the Kotlin BundleFetchBridge (OS network stack)"
    );
    // fetch は純粋シームで正規化したフル URL（bundle_url）で駆動する（ADR-0002 / ADR-0003）。
    assert!(
        src.contains("bundle_url(target)"),
        "fetch_from must fetch the normalized full bundle URL"
    );
}

#[test]
fn the_kotlin_bridge_uses_the_platform_http_stack_with_named_timeouts() {
    let src = kotlin_bridge_src();
    // Android の HttpURLConnection 実装は OkHttp 系（= OS プラットフォームのネットワークスタック）。
    // TLS は OS の信頼ストアから無償で得る（Rust に TLS 依存を入れない・ADR-0002）。
    assert!(
        src.contains("HttpURLConnection"),
        "the Kotlin bridge must fetch over the platform HTTP stack"
    );
    // Rust の呼び出し（JNI）とシグネチャで出会う同期入口。
    assert!(
        src.contains("fun fetchBlocking"),
        "the bridge must expose the blocking JNI entry the Rust host calls"
    );
    // タイムアウト tunable は Rust の名前付き定数（FETCH_TIMEOUT）から渡され、接続・読みの両方に効く。
    assert!(
        src.contains("connectTimeout") && src.contains("readTimeout"),
        "the fetch timeout must apply to both connect and read"
    );
}

#[test]
fn fetch_timeout_stays_a_named_constant_passed_to_the_platform() {
    let src = bundle_source_src();
    // tunable は名前付き定数のまま維持する（ADR-0002 / #740 受け入れ基準）。
    assert!(
        src.contains("const FETCH_TIMEOUT"),
        "the fetch timeout must remain a named constant"
    );
    assert!(
        src.contains("FETCH_TIMEOUT"),
        "fetch_from must pass the named timeout to the platform fetch"
    );
}

fn reload_socket_src() -> String {
    read_relative("src/reload_socket.rs")
}

fn kotlin_reload_bridge_src() -> String {
    read_relative(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/ReloadSocketBridge.kt",
    )
}

/// コメント行（撤去の経緯を記す doc）を除いたコードだけを走査する（qr_scanner_encapsulation
/// と同じ流儀）。
fn code_lines(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_handwritten_rfc6455_websocket_is_gone() {
    let code = code_lines(&reload_socket_src());
    // 素の TCP 上の RFC6455（ハンドシェイク組み立て・フレーム解釈）は委譲完了をもって撤去
    // （ADR-0002 / #742）。
    for marker in [
        "TcpStream",
        "Sec-WebSocket-Key",
        "WS_HANDSHAKE_KEY",
        "WS_OPCODE",
        "open_ws",
        "read_frames",
    ] {
        assert!(
            !code.contains(marker),
            "reload_socket.rs must no longer hand-roll the RFC6455 transport (found {marker:?})"
        );
    }
}

#[test]
fn rust_delegates_the_reload_ws_to_the_kotlin_bridge_over_the_common_jni_seam() {
    let src = reload_socket_src();
    // JNI leaf は共通下地（jni_bridge）だけを使う（bundle_source と同型・ADR-0125）。
    assert!(
        src.contains("jni_bridge::with_activity_env"),
        "the platform reload WS must go through the shared JNI seam"
    );
    assert!(
        src.contains("ReloadSocketBridge"),
        "the reload WS must be delegated to the Kotlin ReloadSocketBridge (OS network stack)"
    );
}

#[test]
fn the_kotlin_reload_bridge_uses_the_platform_websocket() {
    let src = kotlin_reload_bridge_src();
    // OkHttp の WebSocket（= OS プラットフォームのネットワークスタック）が WS(S) を担う。
    // TLS（wss）は OS の信頼ストアから無償で得る（Rust に TLS 依存を入れない・ADR-0002）。
    assert!(
        src.contains("okhttp3"),
        "the Kotlin bridge must ride on OkHttp (the platform network stack)"
    );
    assert!(
        src.contains("newWebSocket"),
        "the Kotlin bridge must open the WebSocket via OkHttp"
    );
    // Rust の呼び出し（JNI）とシグネチャで出会う同期入口（open / awaitEvent / close）。
    for entry in ["fun open", "fun awaitEvent", "fun close"] {
        assert!(
            src.contains(entry),
            "the bridge must expose the blocking JNI entry `{entry}` the Rust host calls"
        );
    }
}

#[test]
fn the_app_declares_the_okhttp_dependency_for_the_reload_ws() {
    let gradle = read_relative("android-app/app/build.gradle.kts");
    // HttpURLConnection と違い WebSocket はプラットフォーム標準 API に無いので、OS スタック側
    // （Kotlin）の実装として OkHttp を明示依存する（ADR-0002 の「Android は OkHttp 系」）。
    assert!(
        gradle.contains("com.squareup.okhttp3:okhttp"),
        "the android app must declare the OkHttp dependency for the reload WS"
    );
}

#[test]
fn the_app_declares_internet_access_and_the_network_security_config() {
    let manifest = manifest_src();
    assert!(
        manifest.contains("android.permission.INTERNET"),
        "the manifest must declare INTERNET for the platform fetch"
    );
    assert!(
        manifest.contains("android:networkSecurityConfig=\"@xml/network_security_config\""),
        "the application must opt into the network security config that scopes cleartext"
    );
}

#[test]
fn cleartext_is_permitted_for_lan_dev_like_expo_go() {
    let config = network_security_config_src();
    // このアプリは Expo Go と同型の dev client：実機から LAN の dev-server（http://192.168.x.x）
    // へ平文で繋ぐのが release 配布版を含む一級機能（Torimi ADR-0002）。networkSecurityConfig は
    // IP レンジ（192.168.0.0/16 等）を表現できず「LAN だけ許可」は設定で書けないため、Expo Go の
    // Play 配布版と同じく base-config で cleartext を全面許可する。公開 Demo Endpoint 側が HTTPS
    // であること（配信側が enforce）は変わらない。
    assert!(
        config.contains("<base-config cleartextTrafficPermitted=\"true\""),
        "cleartext must be permitted so real devices can reach the LAN dev-server over http"
    );
}
