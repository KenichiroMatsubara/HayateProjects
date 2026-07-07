package com.hayateprojects.hayate.adapter_android_demo

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/**
 * App Bundle fetch の Android leaf（ADR-0002 前半・#740）。
 *
 * Miharashi のバンドル取得 transport は OS プラットフォームのネットワークスタックに委譲する：
 * Android の [HttpURLConnection] は OkHttp 系実装で、HTTPS（TLS・OS 信頼ストア・リダイレクト
 * 追従）を無償で供給する。Rust ホスト（`bundle_source.rs`）は正規化済みのフル URL を渡し、
 * fetch 済み JS ソース文字列だけを受け取る——Rust に TLS 依存は入れない（ADR-0002）。
 * LAN dev の平文 http も同じ経路に統一し、cleartext の許可範囲は networkSecurityConfig が
 * LAN dev 用途に限定する。
 *
 * `ErrorOverlayBridge` / `QrScannerBridge` と同じ Rust↔Kotlin JNI seam パターン。
 */
object BundleFetchBridge {
    /**
     * Rust（JNI）用の同期入口。`url` を GET し、応答本文を UTF-8 の JS ソース文字列で返す。
     * **呼び出しスレッドをブロックする**——Rust の boot はネイティブ（非 UI）スレッドで走る
     * 契約（UI スレッドの network は Android が NetworkOnMainThreadException で禁じる）。
     *
     * 失敗（非 200・接続・TLS・タイムアウト）は [IOException] を投げ、Rust 側が例外文言を
     * `BundleFetchError::Platform` に畳んでエラーオーバーレイへ読める形で出す（#530）。
     * timeout（Rust の名前付き定数 `FETCH_TIMEOUT` 由来）は接続・読みの両方に一様に課す。
     */
    @JvmStatic
    fun fetchBlocking(url: String, timeoutMs: Int): String {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.connectTimeout = timeoutMs
        connection.readTimeout = timeoutMs
        // Demo Endpoint / dev-server いずれも GET 一発。リダイレクトは platform 既定で追従する。
        connection.requestMethod = "GET"
        try {
            val status = connection.responseCode
            if (status != HttpURLConnection.HTTP_OK) {
                throw IOException("HTTP $status from $url")
            }
            return connection.inputStream.use { it.readBytes().toString(Charsets.UTF_8) }
        } finally {
            connection.disconnect()
        }
    }
}
