package com.hayateprojects.hayate.adapter_android_demo

import com.google.androidgamesdk.GameActivity

/**
 * Thin GameActivity host for the `hayate-adapter-android` demo (ADR-0094).
 *
 * All application logic lives in Rust: `android-activity`'s game-activity backend
 * invokes `android_main` in the `hayate_adapter_android` cdylib (named via the
 * `android.app.lib_name` meta-data in AndroidManifest.xml). GameActivity is used
 * over NativeActivity solely so GameTextInput can surface the soft-keyboard
 * `InputConnection` to native code for the stage C IME bridge — there is
 * deliberately no Kotlin behaviour here.
 */
class MainActivity : GameActivity()
