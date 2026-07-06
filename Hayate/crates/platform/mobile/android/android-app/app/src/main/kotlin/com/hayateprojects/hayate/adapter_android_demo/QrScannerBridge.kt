package com.hayateprojects.hayate.adapter_android_demo

import android.app.Activity
import android.content.Context
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicReference

/**
 * QR スキャナの Android leaf 実装（ADR-0125）。Google Code Scanner（Play services）でカメラ QR を
 * 1 件読み取る唯一の実装で、2 つの入口を持つ：
 *
 *  - [scanBlocking]：Rust の `MobileQrScanner`（`hayate_adapter_android::qr_scanner::AndroidQrScanner`）
 *    が JNI で呼ぶ。**呼び出しスレッドをブロック**して `String?`（null = キャンセル）を返す。
 *    Rust 側 capability は worker スレッドから呼ぶ契約なので、ここで UI スレッドへ投げて待ってよい。
 *  - [startScan]：Kotlin の bootstrap UI（[DevServerSetupActivity]）が UI スレッドから呼ぶ
 *    コールバック版。ブロックしない。
 *
 * Code Scanner は Play services のスキャナ UI とカメラ取得を内包するので、CameraX も独自カメラ
 * 権限も要らない。実機検証（カメラ起動 → 読み取り）は Play services + 端末が要るため AFK 範囲外。
 *
 * 設計注記：これは「Android 専用のスキャン UI」ではなく **Rust capability の Android leaf**。
 * iOS/Android 横断の統一 API は Rust の `MobileQrScanner` であり、本クラスはその裏で動く Android の
 * 実体（iOS は VisionKit が同じ役を担う）。bootstrap UI も同じ実体を共有する。
 */
object QrScannerBridge {
    /** QR のみ対象の Code Scanner クライアントを作る。 */
    private fun client(activity: Activity) =
        GmsBarcodeScanning.getClient(
            activity,
            GmsBarcodeScannerOptions.Builder()
                .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
                .build(),
        )

    /**
     * UI スレッドからスキャナを起動し、結果（読み取り値 / キャンセル / 失敗）を [onDone] に渡す。
     * キャンセル・失敗はどちらも `null`（capability 契約では「キャンセル = 結果なし」に畳む）。
     */
    private fun launch(activity: Activity, onDone: (String?) -> Unit) {
        client(activity)
            .startScan()
            .addOnSuccessListener { barcode -> onDone(barcode.rawValue) }
            .addOnCanceledListener { onDone(null) }
            .addOnFailureListener { onDone(null) }
    }

    /**
     * Rust capability（JNI）用の同期入口。`activity` 上でスキャナを起動し、結果が出るまで呼び出し
     * スレッドをブロックして返す。**UI スレッドから呼んではならない**（runOnUiThread と await で
     * デッドロックする）— Rust capability は worker スレッドから呼ぶ。
     */
    @JvmStatic
    fun scanBlocking(context: Context): String? {
        // Rust（`ndk_context`）が持つのは Application Context であって Activity ではないため
        // `Context` で受け、Activity は [CurrentActivity] レジストリで解決する。
        val activity = (context as? Activity) ?: CurrentActivity.get() ?: return null
        val latch = CountDownLatch(1)
        val result = AtomicReference<String?>(null)
        activity.runOnUiThread {
            launch(activity) { value ->
                result.set(value)
                latch.countDown()
            }
        }
        latch.await()
        return result.get()
    }

    /** bootstrap UI 用の非ブロック入口（UI スレッドから呼ぶ）。 */
    fun startScan(activity: Activity, onResult: (String?) -> Unit) {
        launch(activity, onResult)
    }
}
