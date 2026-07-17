//! Torimi の Android ホストが App Bundle を取得する源（#532, #740）。
//!
//! ADR-0112 の `app_tsubame` は当初、ビルド時に APK へ焼いた `assets/tsubame.js` を
//! 読んで Hermes に eval していた。Torimi（鳥見）はホストを再ビルドせずバンドルだけ
//! 差し替える dev-client なので、源を**実行時ネットワーク fetch** に替えた（Torimi
//! CONTEXT.md / #528 Web ホストと対称）。eval シームは不変 — 取得した JS ソース String を
//! そのまま `new_hermes_app(.., &bundle)` に渡す。
//!
//! transport は当初「依存追加なし」の素の TCP 上の手書き HTTP/1.1 だったが、公開 Demo
//! Endpoint（HTTPS・ADR-0003）を機に **OS プラットフォームのネットワークスタックへ委譲**した
//! （ADR-0002・#740）。Kotlin 側（`BundleFetchBridge`・Android の `HttpURLConnection` は
//! OkHttp 系実装）が HTTP(S) と TLS を担い、Rust は正規化済みのフル URL（純粋シーム
//! [`bundle_url`]）を渡して fetch 済み JS ソース文字列だけを受け取る。Rust に TLS 依存は
//! 入れない。LAN dev の平文 http も同じ委譲実装に統一し、cleartext の許可範囲は
//! networkSecurityConfig が LAN dev 用途に限定する。

use std::time::Duration;

use crate::dev_server_target::DevServerTarget;

/// dev-server がバンドルを配信する HTTP ルート。`@torimi/dev-server` の `BUNDLE_ROUTE`
/// と一致させる wire 契約（node 依存をネイティブへ持ち込まないため値で複製する）。
/// target が path を持たないとき（従来の `host:port` 入力・既定 target）の落とし先。
const BUNDLE_ROUTE: &str = "/bundle.js";
/// バンドル fetch のタイムアウト。応答しない配信点で永久に待たない上限
/// （#528 Web ホストの `BUNDLE_FETCH_TIMEOUT_MS = 10_000` と対称）。connect / read の両方に
/// 一様に課す（Kotlin 側の `connectTimeout` / `readTimeout` へ渡す）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// バンドル取得の失敗。
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub enum BundleFetchError {
    /// OS スタック（Kotlin 側）が報告した取得失敗。非 200 ステータス（`HTTP 404` 等）・接続・
    /// TLS・タイムアウトの別は、プラットフォームの例外文言をそのまま運ぶ（エラーオーバーレイに
    /// 読める形で出す・#530）。
    Platform(String),
}

#[cfg(target_os = "android")]
pub use platform::{fetch_from, fetch_url};

/// target が指すバンドルのフル URL（純粋シーム・契約テスト対象）。path 無し（`/`）は
/// バンドルルートの wire 契約（[`BUNDLE_ROUTE`]）へ広げ、path 付きはそれをそのまま使う
/// （複数デモをパスで区別する Demo Endpoint・ADR-0003）。scheme 既定ポートの正規化は
/// `dev_server_target::parse` が済ませている（ADR-0002）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn bundle_url(target: &DevServerTarget) -> String {
    let path = match target.path() {
        "/" => BUNDLE_ROUTE,
        path => path,
    };
    format!(
        "{}://{}:{}{}",
        target.scheme().as_str(),
        target.host(),
        target.port(),
        path
    )
}

/// OS スタック委譲の JNI glue（device 専用）。qr_scanner / error_overlay と同じ leaf パターン
/// （`jni::` の直接使用は `jni_bridge` に封じ込め、leaf はそれだけを使う・ADR-0125）。
#[cfg(target_os = "android")]
mod platform {
    use super::*;
    use crate::jni_bridge::JString;

    /// Kotlin の橋渡しクラス（android-app の `BundleFetchBridge`）の JNI 名。
    const BRIDGE_CLASS: &str = "com/hayateprojects/hayate/adapter_android_demo/BundleFetchBridge";
    /// `String url, int timeoutMs` を受けて fetch 済み JS ソース `String` を返す（失敗は Java 例外）。
    const FETCH_METHOD: &str = "fetchBlocking";
    const FETCH_SIG: &str = "(Ljava/lang/String;I)Ljava/lang/String;";

    /// 解決済み [`DevServerTarget`]（端末 UI が入れた URL、無ければ既定）からバンドルを取得する。
    /// `app_tsubame::run` はこれを APK asset 読み込みの代わりに呼び、得た JS ソースをそのまま
    /// `new_hermes_app(.., &bundle)` の eval シームへ渡す。URL は純粋シーム [`bundle_url`] が
    /// 正規化し、実 I/O は OS スタック（Kotlin・ADR-0002）が担う。route / timeout は名前付き定数、
    /// scheme / host / port / path は target が持つ（同じ target が reload 購読も駆動する＝保持・#534）。
    pub fn fetch_from(target: &DevServerTarget) -> Result<String, BundleFetchError> {
        fetch_url(&bundle_url(target))
    }

    /// 任意の URL を OS スタック（Kotlin・ADR-0002）で GET してテキストを返す共通入口。バンドル取得
    /// （[`fetch_from`]）と Demo Manifest 取得（`demo_manifest::fetch_manifest`・#743）が同じ委譲実装・
    /// 同じ名前付き timeout を共有する（手書き HTTP を再導入しない）。
    pub fn fetch_url(url: &str) -> Result<String, BundleFetchError> {
        platform_fetch(url, FETCH_TIMEOUT)
    }

    /// OS のネットワークスタックで `url` を GET し、JS ソース String を受け取る（ADR-0002）。
    /// Kotlin の `BundleFetchBridge.fetchBlocking` を共通 JNI 下地（`jni_bridge`・ADR-0125）経由で
    /// 呼ぶ。呼び出しスレッドをブロックする——boot はネイティブ（非 UI）スレッドで走る契約
    /// （UI スレッドの network は Android が `NetworkOnMainThreadException` で禁じる）。
    /// 失敗（非 200・接続・TLS・タイムアウト）は Java 例外の文言を [`BundleFetchError::Platform`]
    /// に畳んで返す。
    fn platform_fetch(url: &str, timeout: Duration) -> Result<String, BundleFetchError> {
        crate::jni_bridge::with_activity_env(|env, activity| {
            // native スレッドの FindClass はアプリのクラスを見つけられないため、必ずアプリ
            // classloader 経由で解決する（error_overlay / qr_scanner と同じ leaf パターン）。
            let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
            let jurl = match env.new_string(url) {
                Ok(s) => s,
                Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
            };
            let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
            let result = match env
                .call_static_method(
                    &class,
                    FETCH_METHOD,
                    FETCH_SIG,
                    &[(&jurl).into(), timeout_ms.into()],
                )
                .and_then(|value| value.l())
            {
                Ok(obj) => obj,
                Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
            };
            let source: String = match env.get_string(&JString::from(result)) {
                Ok(s) => s.into(),
                Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
            };
            Ok(source)
        })
        .map_err(BundleFetchError::Platform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn tunables_are_named_constants_matching_the_dev_server_contract() {
        // dev-server の BUNDLE_ROUTE（`@torimi/dev-server`）と一致する wire 契約。
        assert_eq!(BUNDLE_ROUTE, "/bundle.js");
        // #528 Web ホストの BUNDLE_FETCH_TIMEOUT_MS = 10_000 と対称。
        assert_eq!(FETCH_TIMEOUT, Duration::from_secs(10));
        // scheme / host / port / path は端末 UI が入れた URL から解決する DevServerTarget が持つ
        // （既定は dev_server_target、#534。フル URL 正規化は #740 / ADR-0002）。
    }

    #[test]
    fn a_target_without_a_path_fetches_the_default_bundle_route() {
        // 従来入力（host:port / path 無し）はバンドルルートの wire 契約（/bundle.js）で取りに行く。
        let target = crate::dev_server_target::resolve(Some("10.0.2.2:5179"));
        assert_eq!(bundle_url(&target), "http://10.0.2.2:5179/bundle.js");
        // 既定 target（未入力）も同じ既定ルート。
        assert_eq!(
            bundle_url(&crate::dev_server_target::resolve(None)),
            "http://10.0.2.2:5179/bundle.js"
        );
    }

    #[test]
    fn a_full_url_target_fetches_exactly_the_entered_path() {
        // 複数デモをパスで区別する Demo Endpoint（ADR-0003）：貼られたフル URL の path を
        // そのまま fetch する（scheme 既定ポートは URL 正規化・dev_server_target が契約テスト済み）。
        let target =
            crate::dev_server_target::resolve(Some("https://demo.example/solid/bundle.js"));
        assert_eq!(
            bundle_url(&target),
            "https://demo.example:443/solid/bundle.js"
        );

        let lan = crate::dev_server_target::resolve(Some("192.168.1.5:8080/react/bundle.js"));
        assert_eq!(bundle_url(&lan), "http://192.168.1.5:8080/react/bundle.js");
    }
}
