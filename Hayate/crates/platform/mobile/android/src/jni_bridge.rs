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
pub(crate) use jni::objects::JString;

/// android-activity が保持する JavaVM に現在スレッドを attach し、Activity(Context) と
/// `JNIEnv` をクロージャへ渡す。JNI 参照はクロージャのスコープ内でのみ有効——外へ持ち出さない
/// こと。個々の JNI leaf（`qr_scanner.rs` / `error_overlay.rs`）はこれだけを使う。
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
