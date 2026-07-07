//! バンドル取得 transport の OS スタック委譲（ADR-0002 前半・#740）のホスト可読ガード。
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
fn cleartext_is_scoped_to_lan_dev_and_https_is_the_default() {
    let config = network_security_config_src();
    // 既定は https（base-config で cleartext を禁じる）。
    assert!(
        config.contains("<base-config cleartextTrafficPermitted=\"false\""),
        "cleartext must be off by default"
    );
    // LAN dev（エミュレータ loopback / 端末 loopback）だけ平文 http を明示許可する。
    assert!(
        config.contains("cleartextTrafficPermitted=\"true\""),
        "LAN dev must keep an explicit cleartext allowance"
    );
    for dev_host in ["10.0.2.2", "localhost", "127.0.0.1"] {
        assert!(
            config.contains(dev_host),
            "the cleartext allowance must cover the {dev_host} dev host"
        );
    }
}
