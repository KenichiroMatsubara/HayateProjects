//! Android leaf の QR スキャナ実装（ADR-0125）。
//!
//! 契約の正本 [`QrScanner`]（`hayate_core`）を満たす Android 実装。バックエンドは Google Code
//! Scanner（`com.google.android.gms:play-services-code-scanner`）— Play services が full-screen の
//! スキャナ UI とカメラ取得を持つので、CameraX も独自カメラ権限も要らない。ただし Code Scanner は
//! **Play services の Kotlin/Java API しか無く NDK 経路が無い**ため、`audio_output`（純 NDK）と違い
//! 本 leaf は**本アプリ初の Rust↔Kotlin JNI seam**になる（QR デコードを純 native で持つには独自
//! デコーダ実装が要り非現実的・ADR-0125）。JNI glue は `#[cfg(target_os="android")]` に封じ込め、
//! Kotlin 側の実体は `QrScannerBridge`（android-app）に置く。
//!
//! Web は family-of-1 として別 leaf（`@miharashi/host-web` の `scanQrFromCamera`、`BarcodeDetector`）
//! を直接持つ（ADR-0117）。本 leaf と iOS leaf（VisionKit・未実装）は `MobileQrScanner` facade に
//! 載り、上位は iOS/Android を単一 API で扱う。
//!
//! 実機検証（カメラ起動 → 読み取り）は Play services + 端末が要るため AFK 範囲外。`audio_output` と
//! 同じく、汚い FFI glue は薄く保ち device ビルドでのみコンパイルする。

#[cfg_attr(not(target_os = "android"), allow(unused_imports))]
pub use hayate_core::{QrScanner, ScannedCode};

#[cfg(target_os = "android")]
pub use platform::AndroidQrScanner;

#[cfg(target_os = "android")]
mod platform {
    use super::*;
    use crate::jni_bridge::JString;
    use hayate_core::capability::CapabilityError;

    /// Kotlin の橋渡しクラス（android-app の `QrScannerBridge`）の JNI 名と署名。`scanBlocking` は
    /// 渡した Activity 上で Code Scanner を起動し、読み取り（or キャンセル）まで**呼び出しスレッドを
    /// ブロック**して `String?`（null = キャンセル）を返す。呼び側（capability 利用者）は UI スレッド
    /// 以外から呼ぶ前提（`QrScanner` 契約どおり）。
    const BRIDGE_CLASS: &str = "com/hayateprojects/hayate/adapter_android_demo/QrScannerBridge";
    const BRIDGE_METHOD: &str = "scanBlocking";
    // `ndk_context` の Context は Application（Activity ではない）ため、Kotlin 側は `Context` で
    // 受けて Activity を `CurrentActivity` レジストリで解決する（error_overlay.rs と同じ理由）。
    const BRIDGE_SIG: &str = "(Landroid/content/Context;)Ljava/lang/String;";

    /// Code Scanner（Play services）でカメラ QR を 1 件読む Android leaf。役割名は
    /// 「QR/バーコードのスキャナ UI」。具体バックエンドは Google Code Scanner。
    #[derive(Default)]
    pub struct AndroidQrScanner;

    /// JNI 呼び出しの実行時失敗を `CapabilityError::Platform` に写す。
    fn platform_err(message: impl ToString) -> CapabilityError {
        CapabilityError::Platform {
            code: 0,
            message: message.to_string(),
        }
    }

    impl QrScanner for AndroidQrScanner {
        fn scan(&mut self) -> Result<Option<ScannedCode>, CapabilityError> {
            // JavaVM/Activity への attach は共通下地（`jni_bridge`）に任せる（capability は worker
            // スレッドから呼ばれる前提。`jni_bridge::with_activity_env` が現在スレッドを attach する）。
            crate::jni_bridge::with_activity_env(|env, activity| {
                // native スレッドの FindClass はアプリのクラスを見つけられないため、
                // アプリ classloader 経由で解決する（`jni_bridge::app_class` のコメント参照）。
                let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
                // Kotlin: Code Scanner を UI スレッドで起動し、結果まで native スレッドをブロックして返す。
                let result = match env
                    .call_static_method(&class, BRIDGE_METHOD, BRIDGE_SIG, &[activity.into()])
                {
                    Ok(r) => r,
                    Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
                };
                let obj = result.l().map_err(|e| e.to_string())?;
                if obj.is_null() {
                    // ユーザがスキャナを閉じた（キャンセル）。
                    return Ok(None);
                }
                let value: String = env
                    .get_string(&JString::from(obj))
                    .map_err(|e| e.to_string())?
                    .into();
                Ok(Some(ScannedCode { value }))
            })
            .map_err(platform_err)
        }
    }
}
