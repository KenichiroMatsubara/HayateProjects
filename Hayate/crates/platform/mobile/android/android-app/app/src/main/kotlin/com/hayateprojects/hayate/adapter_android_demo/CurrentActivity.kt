package com.hayateprojects.hayate.adapter_android_demo

import android.app.Activity
import java.lang.ref.WeakReference
import java.util.concurrent.atomic.AtomicReference

/**
 * Rust から JNI で届く Context を Activity に解決するための最小レジストリ。
 *
 * `android-activity`（game-activity backend）が `ndk_context` に設定するのは **Application**
 * であって Activity ではない（android-activity 0.6 `init.rs` の `initialize_android_context`）。
 * 一方、エラーオーバーレイ（`ErrorOverlayBridge`）や QR スキャナ（`QrScannerBridge`）は
 * `runOnUiThread` / View 追加 / スキャナ UI 起動に Activity 実体を要する。そこで前面の
 * `MainActivity` が自身をここへ登録し、JNI ブリッジは受け取った Context が Activity で
 * なければここから引く。
 *
 * WeakReference なのは Activity の破棄を妨げないため（レジストリが原因のリークを作らない）。
 */
object CurrentActivity {
    private val ref = AtomicReference<WeakReference<Activity>>(WeakReference(null))

    fun set(activity: Activity) {
        ref.set(WeakReference(activity))
    }

    fun get(): Activity? = ref.get().get()
}
