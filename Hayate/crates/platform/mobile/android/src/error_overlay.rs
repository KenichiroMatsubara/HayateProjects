//! ネイティブ View によるエラーオーバーレイの JNI ブリッジ（#530 系）。
//!
//! Hayate（要素ツリー→GPU パイプライン）に一切依存せず、boot 失敗・panic を画面に出す——Web
//! ホストの生 DOM error panel（`Miharashi/host-web` の built-in error panel）と対称の「潰れない
//! 土台」。エラー報告の仕組みが、報告対象になり得るサブシステム（Hayate 自身の初期化・GPU
//! surface・レンダリング）に依存してはいけない、という原則（Vite のエラーオーバーレイ・React
//! の Error Boundary と同じ）。Kotlin 側の素の Android View（`ErrorOverlayBridge`）を、共通下地
//! （`jni_bridge`）経由で呼ぶ——`qr_scanner.rs` と同じ JNI leaf パターン（`jni::`/
//! `ndk_context::` の直接使用は `jni_bridge.rs` 1 つに封じ込め、leaf 側はそれだけを使う）。

/// Kotlin の橋渡しクラス（android-app の `ErrorOverlayBridge`）の JNI 名。
const BRIDGE_CLASS: &str = "com/hayateprojects/hayate/adapter_android_demo/ErrorOverlayBridge";
// `ndk_context` が渡してくるのは Application Context（Activity ではない。android-activity 0.6
// の `initialize_android_context` 参照）ため、Kotlin 側は `Context` で受けて Activity を
// `CurrentActivity` レジストリで解決する。
const SHOW_METHOD: &str = "showError";
const SHOW_SIG: &str = "(Landroid/content/Context;Ljava/lang/String;)V";
const CLEAR_METHOD: &str = "clearError";
const CLEAR_SIG: &str = "(Landroid/content/Context;)V";

/// 画面に明示エラーメッセージのオーバーレイを出す。boot 失敗ハンドリング（`app_tsubame::run`）・
/// panic hook（`app::install_panic_logger`）の両方から呼ぶ。Hayate/GPU パイプラインを一切
/// 経由しないため、それらの初期化・描画が壊れていても呼べる。JNI 呼び出し自体の失敗
/// （Activity 未確立・JNI 環境未アタッチ等）はログにだけ残し、呼び出し元をパニックさせない
/// （エラー表示の失敗が新たなクラッシュを生んではいけない）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn show_error(message: &str) {
    if let Err(err) = call(SHOW_METHOD, SHOW_SIG, Some(message)) {
        log::error!("hayate-adapter-android: error overlay 表示に失敗しました: {err}");
    }
}

/// オーバーレイを消す（boot 成功時。Web ホストの `clearBuiltinErrorPanel` と対称）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn clear_error() {
    if let Err(err) = call(CLEAR_METHOD, CLEAR_SIG, None) {
        log::error!("hayate-adapter-android: error overlay 消去に失敗しました: {err}");
    }
}

/// `ErrorOverlayBridge` の static method を 1 つ呼ぶ薄いグルー。`message` が `Some` なら 2 引数版
/// （`showError`）、`None` なら 1 引数版（`clearError`）を呼ぶ。attach は `jni_bridge` に任せる。
fn call(method: &str, sig: &str, message: Option<&str>) -> Result<(), String> {
    crate::jni_bridge::with_activity_env(|env, activity| {
        // native スレッドの FindClass はアプリのクラスを見つけられない（システム classloader に
        // 落ちる）ため、必ずアプリ classloader 経由で解決する。
        let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
        let result = match message {
            Some(text) => {
                let jtext = match env.new_string(text) {
                    Ok(s) => s,
                    Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
                };
                env.call_static_method(&class, method, sig, &[activity.into(), (&jtext).into()])
            }
            None => env.call_static_method(&class, method, sig, &[activity.into()]),
        };
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(crate::jni_bridge::describe_java_error(env, e)),
        }
    })
}
