package com.hayateprojects.hayate.adapter_android_demo

import android.os.Bundle
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
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
 *  - SurfaceView をシステムバーの安全領域内に収めるインセット適用（下記）。
 */
class MainActivity : GameActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // super.onCreate が native ライブラリをロードして android_main を始動するため、
        // Rust 側から見える前に登録しておく。
        CurrentActivity.set(this)
        super.onCreate(savedInstanceState)
        fitSurfaceViewToSafeArea()
    }

    /**
     * GameActivity の SurfaceView はフルウィンドウ（edge-to-edge）に配置され、実機では
     * ステータスバー/ナビゲーションバーの裏まで広がる。`AndroidApp::content_rect()` も
     * フルウィンドウを返すため（インセットは onContentRectChanged ではなく WindowInsets
     * 経由でしか届かない）、Rust 側だけでは補正できない——最下点がナビゲーションバー分
     * だけ固定ピクセルずれて到達不能になるバグの根本原因。
     *
     * WindowInsets の systemBars + displayCutout を SurfaceView のマージンとして適用し、
     * ANativeWindow 自体を安全領域サイズにする。これで Rust の window 寸法・レイアウト
     * ビューポート・タッチ座標が全て安全領域基準で自動的に一致する。インセットは消費
     * しない（GameActivity 自身が SurfaceView 上のリスナーで GameTextInput の IME
     * インセットを処理するため、下流へ流す）。
     */
    private fun fitSurfaceViewToSafeArea() {
        val content = findViewById<ViewGroup>(android.R.id.content)
        val surface: View = content.getChildAt(0) ?: return
        ViewCompat.setOnApplyWindowInsetsListener(content) { _, insets ->
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
            )
            val lp = surface.layoutParams as ViewGroup.MarginLayoutParams
            if (lp.leftMargin != bars.left || lp.topMargin != bars.top ||
                lp.rightMargin != bars.right || lp.bottomMargin != bars.bottom
            ) {
                lp.setMargins(bars.left, bars.top, bars.right, bars.bottom)
                surface.layoutParams = lp
            }
            insets
        }
    }

    /**
     * GameActivity はタッチを SurfaceView ではなく **Activity.onTouchEvent（ウィンドウ座標）**
     * で受けてネイティブへ流す。SurfaceView を安全領域マージンで下げた分、描画とタッチの
     * 座標系がずれる（タッチが systemBars.top 分だけ下に着弾する）ため、ネイティブへ渡す前に
     * MotionEvent を SurfaceView 相対へ平行移動して描画と一致させる。
     */
    override fun onTouchEvent(event: MotionEvent): Boolean =
        super.onTouchEvent(offsetToSurface(event))

    override fun onGenericMotionEvent(event: MotionEvent): Boolean =
        super.onGenericMotionEvent(offsetToSurface(event))

    private fun offsetToSurface(event: MotionEvent): MotionEvent {
        val surface: View = findViewById<ViewGroup>(android.R.id.content).getChildAt(0)
            ?: return event
        val loc = IntArray(2)
        surface.getLocationInWindow(loc)
        event.offsetLocation(-loc[0].toFloat(), -loc[1].toFloat())
        return event
    }
}
