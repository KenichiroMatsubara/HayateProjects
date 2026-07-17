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

/// Kotlin（`MainActivity.nativePushSafeAreaInsets`）→ Rust の JNI エクスポート（edge-to-edge /
/// b2, issue #794・ADR-0144）。WindowInsets（systemBars + displayCutout、物理px）を受け取り、
/// フレームループ（`app.rs`）が読むグローバル（`safe_area`）へ格納する。JNI 封じ込め方針
/// （`qr_scanner_encapsulation.rs`）に従い、`jni::` を直接使える唯一のファイルであるここに置く。
/// シンボル名は `com.hayateprojects.hayate.adapter_android_demo.MainActivity` の JNI 変換
/// （パッケージ名の `_` は `_1` にエスケープ）。`hayate_adapter_android` cdylib が export し、
/// GameActivity がロードした後 Kotlin の `external fun` から呼ばれる。
#[no_mangle]
pub extern "system" fn Java_com_hayateprojects_hayate_adapter_1android_1demo_MainActivity_nativePushSafeAreaInsets<
    'local,
>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    left: jni::sys::jint,
    top: jni::sys::jint,
    right: jni::sys::jint,
    bottom: jni::sys::jint,
) {
    // jint は i32。systemBars + displayCutout の物理px（IME は含まない）。
    crate::safe_area::store_pushed_insets(left, top, right, bottom);
}

/// Kotlin（`MainActivity.nativePushRenderConfig`）→ Rust の JNI エクスポート（描画バックエンド /
/// AA 方式のランタイム切替, issue #795・ADR-0145）。intent extra 由来の上書き文字列（未指定は空文字）
/// を受け取り、`render_config` のグローバルへ格納する（`init_gpu_surface` が読む）。JNI 封じ込め
/// 方針に従いここに置く。
#[no_mangle]
pub extern "system" fn Java_com_hayateprojects_hayate_adapter_1android_1demo_MainActivity_nativePushRenderConfig<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    backend: JString<'local>,
    aa: JString<'local>,
) {
    let backend = jstring_to_owned(&mut env, &backend);
    let aa = jstring_to_owned(&mut env, &aa);
    crate::render_config::store_pushed_config(&backend, &aa);
}

/// Kotlin（`MainActivity.nativePushRendererConfig`）→ Rust の JNI エクスポート（レンダラ
/// （vello/skia）の実行時強制指定, issue #802・ADR-0146/0147、および skia 内 surface
/// （raster/GL）の切替, issue #803・ADR-0146 §3）。intent extra 由来の上書き文字列
/// （未指定は空文字）を受け取り、`renderer_config` のグローバルへ格納する
/// （`init_and_spawn_raster` が読む）。JNI 封じ込め方針に従いここに置く。
#[no_mangle]
pub extern "system" fn Java_com_hayateprojects_hayate_adapter_1android_1demo_MainActivity_nativePushRendererConfig<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    renderer: JString<'local>,
    skia_surface: JString<'local>,
) {
    let renderer = jstring_to_owned(&mut env, &renderer);
    crate::renderer_config::store_pushed_renderer(&renderer);
    let skia_surface = jstring_to_owned(&mut env, &skia_surface);
    crate::renderer_config::store_pushed_skia_surface(&skia_surface);
}

/// `JString` を Rust の `String` に写す。null / 変換失敗は空文字（未指定扱い → 既定へ）。
fn jstring_to_owned(env: &mut JNIEnv<'_>, s: &JString<'_>) -> String {
    env.get_string(s).map(|js| js.into()).unwrap_or_default()
}

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
        Err(e) => Err(format!(
            "{dotted} の解決に失敗: {}",
            describe_java_error(env, e)
        )),
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
