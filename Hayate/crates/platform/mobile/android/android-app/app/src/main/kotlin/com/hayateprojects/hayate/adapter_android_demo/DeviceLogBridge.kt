package com.hayateprojects.hayate.adapter_android_demo

import android.os.Build
import java.net.HttpURLConnection
import java.net.URL
import java.util.UUID

/**
 * Device Log 送信の Android leaf（#787・ADR-0005）。**device 未検証**。
 *
 * Torimi の Device Log は USB/adb 無しにホストのログを開発機の Dev Server へ届ける（CONTEXT.md
 * 「Device Log」）。送信 transport は bundle fetch / reload と同じく OS スタックへ委譲する：Rust の
 * 純粋シーム（`device_log.rs`）がバッファ・seq 採番・バッチ化・wire JSON 化を所有し、Kotlin は
 * 組み上がった JSON body を `POST <logRoutePrefix><deviceId>` するだけの薄い I/O ポートに徹する。
 *
 * `BundleFetchBridge` / `ReloadSocketBridge` と同じ Rust↔Kotlin JNI seam パターン。
 */
object DeviceLogBridge {
    /**
     * Rust（JNI）用の同期入口。`jsonBody` を `url` へ `POST` し、成功可否を返す。**呼び出しスレッドを
     * ブロックする**——送信は Rust のネイティブ（非 UI）スレッドで駆動する契約（UI スレッドの network は
     * Android が NetworkOnMainThreadException で禁じる）。
     *
     * 失敗（非 2xx・接続・タイムアウト）は例外にせず `false` を返す：送り側は失敗をリングバッファ
     * 保持＋次間隔の再送で吸収する（#788）ので、例外で boot/描画を止めない。at-least-once の重複は
     * 受け側（#785）が `(deviceId, seq)` で捨てる。
     */
    @JvmStatic
    fun postBlocking(url: String, jsonBody: String, timeoutMs: Int): Boolean {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.connectTimeout = timeoutMs
        connection.readTimeout = timeoutMs
        connection.requestMethod = "POST"
        connection.doOutput = true
        connection.setRequestProperty("Content-Type", "application/json; charset=utf-8")
        return try {
            connection.outputStream.use { it.write(jsonBody.toByteArray(Charsets.UTF_8)) }
            val status = connection.responseCode
            // dev-server は受理を 204 で返す（#785）。2xx を成功とみなす。
            status in 200..299
        } catch (_: Exception) {
            false
        } finally {
            connection.disconnect()
        }
    }

    /**
     * 不透明なランダム Device ID（インストール単位・ハードウェア由来でない・ADR-0005）。初回起動時に
     * Rust が呼んで生成し、以降はローカル永続化した値を使う（`device_log::load_or_create_device_id`）。
     */
    @JvmStatic
    fun randomId(): String = UUID.randomUUID().toString()

    /** 表示用の端末ラベル（端末モデル名）。各バッチのペイロードに載る Device Label（#787）。 */
    @JvmStatic
    fun deviceLabel(): String = Build.MODEL ?: "Android"
}
