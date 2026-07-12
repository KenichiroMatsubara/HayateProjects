package com.hayateprojects.hayate.adapter_android_demo

import android.os.Bundle
import android.util.Log
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.google.androidgamesdk.GameActivity

/**
 * Thin GameActivity host for the `hayate-adapter-android` demo (ADR-0094).
 *
 * All application logic lives in Rust: `android-activity`'s game-activity backend
 * invokes `android_main` in the `hayate_adapter_android` cdylib (named via the
 * `android.app.lib_name` meta-data in AndroidManifest.xml). GameActivity is used
 * over NativeActivity solely so GameTextInput can surface the soft-keyboard
 * `InputConnection` to native code for the stage C IME bridge.
 *
 * Kotlin 挙動は 2 つ:
 *  - [CurrentActivity] への自己登録：Rust の JNI ブリッジ（エラーオーバーレイ・
 *    QR スキャナ）が Application Context しか持たないため、Activity 実体をここで供給する。
 *  - **edge-to-edge ＋ WindowInsets の JNI push（b2, issue #794・ADR-0144）**：SurfaceView は
 *    フルウィンドウのまま（マージンで縮めない）、systemBars + displayCutout のインセットを Rust へ
 *    push し、安全領域処理（レイアウトビューポート縮小・シーン平行移動・タッチ座標補正）は
 *    アダプタ内（Rust）で完結する。旧マージン方式は Nothing Phone 3a でリスナーが端末依存で
 *    不発になりステータスバー侵食を起こしたため撤去した。
 */
class MainActivity : GameActivity() {
    private companion object {
        /**
         * ステータスバーのアイコンを暗色（明るい背景向け）にするか。b2 では静的に固定する
         * （アプリテーマからの動的導出は将来の別 issue）。ルート背景は暗色系
         * （`STAGE_A_CLEAR_COLOR`）なので、明色アイコン＝ `isAppearanceLightStatusBars = false`。
         */
        const val LIGHT_STATUS_BAR_ICONS = false

        /** logcat タグ（端末別のインセット配送問題の診断用）。 */
        const val TAG = "HayateSafeArea"

        /**
         * 描画バックエンド（Vulkan/GL）・AA 方式（Area/MSAA8/MSAA16）の実行時上書き intent extra
         * キー（issue #795・ADR-0145）。`adb shell am start -e hayate.backend gl -e hayate.aa msaa8`
         * で再ビルドなしに切り替える。未指定は空文字で push し、Rust 側で既定へ落とす。
         */
        const val BACKEND_EXTRA = "hayate.backend"
        const val AA_EXTRA = "hayate.aa"

        /**
         * レンダラ（vello/skia）の実行時強制指定 intent extra キー（issue #802・
         * ADR-0146/0147）。`adb shell am start -e hayate.renderer skia` で APK を作り直さずに
         * vello ⇄ skia を切り替える（#795 の `hayate.backend`/`hayate.aa` と同じ操作感）。
         * 未指定は空文字で push し、Rust 側（`renderer_config` → Renderer Selection Policy）で
         * 既定順序（vello → skia の一方向 fallback）へ落とす。
         */
        const val RENDERER_EXTRA = "hayate.renderer"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // super.onCreate が native ライブラリをロードして android_main を始動するため、
        // Rust 側から見える前に登録しておく。
        CurrentActivity.set(this)
        super.onCreate(savedInstanceState)
        // GPU surface 初期化（CreateSurface）より前に描画設定の上書きを Rust へ届ける。
        pushRenderConfig()
        pushRendererOverride()
        enableEdgeToEdge()
        installInsetPush()
    }

    /**
     * 描画バックエンド / AA 方式の実行時上書き（intent extra）を Rust へ push する（#795）。
     * 未指定（extra 無し）は空文字で渡し、Rust 側（`render_config`）で既定（Vulkan・Area）へ
     * 落とす。APK を作り直さずに 3 実験（MSAA8/16・GL）を回すための口。
     */
    private fun pushRenderConfig() {
        val backend = intent.getStringExtra(BACKEND_EXTRA) ?: ""
        val aa = intent.getStringExtra(AA_EXTRA) ?: ""
        Log.i(TAG, "render config override: backend=\"$backend\" aa=\"$aa\"")
        nativePushRenderConfig(backend, aa)
    }

    /**
     * レンダラ（vello/skia）の実行時強制指定（intent extra）を Rust へ push する（issue #802）。
     * 未指定（extra 無し）は空文字で渡し、Rust 側（`renderer_config` → Renderer Selection
     * Policy）で既定順序（vello → skia の一方向 fallback）へ落とす。GPU surface 初期化
     * （CreateSurface）より前に届ける必要があるため `pushRenderConfig()` と同じ場所で呼ぶ。
     */
    private fun pushRendererOverride() {
        val renderer = intent.getStringExtra(RENDERER_EXTRA) ?: ""
        Log.i(TAG, "renderer override: \"$renderer\"")
        nativePushRendererConfig(renderer)
    }

    /**
     * SurfaceView をシステムバー/ディスプレイカットアウトの裏までフルウィンドウに広げる（b2）。
     * 安全領域の処理は Rust 側（アダプタ内）で完結するので、ここではバーの裏まで描けるように
     * するだけ。ステータスバーのアイコン色は名前付き定数で静的に設定する。
     */
    private fun enableEdgeToEdge() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowCompat.getInsetsController(window, window.decorView).isAppearanceLightStatusBars =
            LIGHT_STATUS_BAR_ICONS
    }

    /**
     * WindowInsets（systemBars + displayCutout）を JNI で Rust へ push する。リスナー発火ごとに
     * 加え、リスナー不発端末（Nothing Phone 3a 実例）への保険として onCreate 後に
     * `rootWindowInsets` スナップショットも一度 push する。インセットは消費しない
     * （GameActivity 自身が SurfaceView 上のリスナーで GameTextInput の IME インセットを処理
     * するため、下流へ流す）。
     */
    private fun installInsetPush() {
        val content = findViewById<ViewGroup>(android.R.id.content)
        ViewCompat.setOnApplyWindowInsetsListener(content) { _, insets ->
            pushInsets(insets)
            insets // 消費しない（下流へ流す）
        }
        // リスナーが発火しない端末への保険。レイアウト確定後に一度スナップショットを push。
        content.post { ViewCompat.getRootWindowInsets(content)?.let { pushInsets(it) } }
    }

    private fun pushInsets(insets: WindowInsetsCompat) {
        // systemBars + displayCutout。IME インセット（ソフトキーボード）は含めない
        // — GameTextInput が別途処理する。
        val bars = insets.getInsets(
            WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
        )
        Log.i(
            TAG,
            "WindowInsets → Rust: left=${bars.left} top=${bars.top} " +
                "right=${bars.right} bottom=${bars.bottom}",
        )
        nativePushSafeAreaInsets(bars.left, bars.top, bars.right, bars.bottom)
    }

    /**
     * インセット（物理px）を Rust（`jni_bridge.rs` の
     * `Java_..._nativePushSafeAreaInsets`）へ push する。実体は
     * `hayate_adapter_android` cdylib が export する。
     */
    private external fun nativePushSafeAreaInsets(left: Int, top: Int, right: Int, bottom: Int)

    /**
     * 描画バックエンド / AA 方式の上書き（intent extra 由来、未指定は空文字）を Rust
     * （`jni_bridge.rs` の `Java_..._nativePushRenderConfig`）へ push する（#795）。
     */
    private external fun nativePushRenderConfig(backend: String, aa: String)

    /**
     * レンダラ（vello/skia）の強制指定（intent extra 由来、未指定は空文字）を Rust
     * （`jni_bridge.rs` の `Java_..._nativePushRendererConfig`）へ push する（issue #802）。
     */
    private external fun nativePushRendererConfig(renderer: String)
}
