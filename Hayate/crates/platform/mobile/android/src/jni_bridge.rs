//! Rust↔Kotlin JNI の共通下地（ADR-0125 の「封じ込め」方針の一般化）。
//!
//! 元々は QR スキャナ（`qr_scanner.rs`）が本アプリ唯一の JNI seam で、`jni::`/`ndk_context::` の
//! 直接使用をそのファイル 1 つに封じ込めるホストテスト（`tests/qr_scanner_encapsulation.rs`）が
//! あった。エラーオーバーレイ（`error_overlay.rs`）という 2 つ目の JNI leaf を足すにあたり、
//! 「1 ファイルに封じ込める」という方針自体は維持しつつ、対象を「JNI leaf 全部」からこの共通
//! 下地 1 つに絞った——各 leaf は `with_activity_env` だけを使い、`jni::`/`ndk_context::` を
//! 直接触らない。封じ込めテストも本ファイルだけを許可するよう追随する。

use jni::objects::JObject;
use jni::{JNIEnv, JavaVM};

// JNI leaf（`qr_scanner.rs` / `error_overlay.rs`）が値の受け渡しに要る型の再エクスポート。
// `jni::` の直接使用をこのファイルへ封じ込めるため、leaf 側はこちら経由で参照する。
pub(crate) use jni::objects::{JClass, JString};

/// android-activity が保持する JavaVM に現在スレッドを attach し、Context と `JNIEnv` を
/// クロージャへ渡す。JNI 参照はクロージャのスコープ内でのみ有効——外へ持ち出さないこと。
/// 個々の JNI leaf（`qr_scanner.rs` / `error_overlay.rs`）はこれだけを使う。
///
/// 注意: この Context は **Application** であって Activity ではない（android-activity 0.6 の
/// `init.rs` が Application のグローバル参照を `ndk_context` に設定する）。Activity 実体が
/// 要る Kotlin ブリッジは `Context` で受け、`CurrentActivity` レジストリで解決すること——
/// `Landroid/app/Activity;` のシグネチャで渡すと CheckJNI の "bad arguments" abort で落ちる。
pub(crate) fn with_activity_env<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let ctx = ndk_context::android_context();
    // SAFETY: `android-activity` が `ndk_context` に設定した生ポインタで、アプリ生存中は有効。
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    // SAFETY: 同上（Activity(Context) の生ポインタ）。
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };
    f(&mut env, &activity)
}

/// アプリのクラスを JNI で解決する。native スレッド（Rust のゲームループ・worker）から
/// `FindClass`（= `call_static_method` に文字列クラス名を渡す経路）を使うと、Java フレームが
/// 無いためシステム classloader に落ち、アプリの Kotlin クラスは `ClassNotFoundException` に
/// なる（JNI の既知の罠）。Activity の `getClassLoader().loadClass(...)` 経由なら
/// どのスレッドからでも解決できる。JNI leaf はクラス名を直接 `call_static_method` へ渡さず、
/// 必ずこれで解決した `JClass` を使うこと。
pub(crate) fn app_class<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject<'_>,
    jni_name: &str,
) -> Result<JClass<'local>, String> {
    let loader = match env
        .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(|v| v.l())
    {
        Ok(loader) => loader,
        Err(e) => return Err(describe_java_error(env, e)),
    };
    // loadClass はドット区切りの binary name を取る（JNI 名はスラッシュ区切り）。
    let dotted = jni_name.replace('/', ".");
    let jname = match env.new_string(&dotted) {
        Ok(s) => s,
        Err(e) => return Err(describe_java_error(env, e)),
    };
    match env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[(&jname).into()],
        )
        .and_then(|v| v.l())
    {
        Ok(class) => Ok(JClass::from(class)),
        Err(e) => Err(format!("{dotted} の解決に失敗: {}", describe_java_error(env, e))),
    }
}

/// JNI エラーを文字列化する。Java 例外が保留中なら、スタックトレースを logcat（System.err）へ
/// 出してからクリアする——「Java exception was thrown」の一行だけで原因が闇に消えるのを防ぎ、
/// かつ例外を保留したまま次の JNI 呼び出しへ進んで abort するのも防ぐ。
pub(crate) fn describe_java_error(env: &mut JNIEnv<'_>, err: jni::errors::Error) -> String {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        format!("{err}（Java 例外のスタックトレースは logcat の System.err に出力）")
    } else {
        err.to_string()
    }
}
