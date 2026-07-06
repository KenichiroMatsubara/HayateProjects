package com.hayateprojects.hayate.adapter_android_demo

import android.os.Bundle
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
 * 唯一の Kotlin 挙動は [CurrentActivity] への自己登録：Rust の JNI ブリッジ
 * （エラーオーバーレイ・QR スキャナ）が Application Context しか持たないため、
 * Activity 実体をここで供給する。
 */
class MainActivity : GameActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // super.onCreate が native ライブラリをロードして android_main を始動するため、
        // Rust 側から見える前に登録しておく。
        CurrentActivity.set(this)
        super.onCreate(savedInstanceState)
    }
}
