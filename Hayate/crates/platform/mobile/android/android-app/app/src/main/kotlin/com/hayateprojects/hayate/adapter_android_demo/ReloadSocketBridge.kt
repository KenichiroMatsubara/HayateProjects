package com.hayateprojects.hayate.adapter_android_demo

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

/**
 * reload 購読 WS の Android leaf（ADR-0002 後半・#742）。
 *
 * Miharashi の reload 購読 transport は OS プラットフォームのネットワークスタックに委譲する：
 * OkHttp の [WebSocket] が WS(S)（TLS・OS 信頼ストア）を無償で供給する。Rust ホスト
 * （`reload_socket.rs`）は URL（https 由来なら wss）を渡して [open] し、[awaitEvent] で受信
 * イベントだけを受け取る——Rust に TLS 依存は入れない（ADR-0002）。reload の意味づけ
 * （`reload` メッセージで full reload・切断時の backoff 再接続）は Rust 純粋シーム
 * （`miharashi_reload`）の領分で、ここは受けたものを右から左へ渡すだけ（HMR を解さない・
 * ADR-0001）。
 *
 * [BundleFetchBridge] / [ErrorOverlayBridge] と同じ Rust↔Kotlin JNI seam パターン。OkHttp の
 * listener コールバックは OkHttp のスレッドで届くので、per-handle の [LinkedBlockingQueue] に
 * 積み、Rust の背景スレッドが [awaitEvent] でブロッキング取り出しする（Rust main へは mpsc で
 * 渡る——単一スレッド契約は Rust 側が守る）。
 */
object ReloadSocketBridge {
    /** テキストフレームイベントの prefix（wire 契約：Rust `reload_socket.rs` と値で一致させる）。 */
    private const val EVENT_TEXT_PREFIX = "text\t"

    /** 切断イベント（接続失敗・TLS 失敗・サーバ close のすべてを畳む。Rust は backoff 再接続する）。 */
    private const val EVENT_CLOSED = "closed"

    /** WS 正常クローズのステータスコード（RFC 6455 の 1000）。 */
    private const val NORMAL_CLOSURE_CODE = 1000

    /** WS 接続確立のタイムアウト（ms）。応答しない配信点で永久に待たない上限。 */
    private const val CONNECT_TIMEOUT_MS = 10_000L

    /**
     * 読みタイムアウト（ms）。reload WS は「シグナルが来るまで何も流れない」長寿命接続なので
     * 0（無効）にする——切断検知は [PING_INTERVAL_MS] の keepalive が担う。
     */
    private const val READ_TIMEOUT_MS = 0L

    /** keepalive ping の間隔（ms）。NAT / アイドル切断を検知して onFailure → backoff 再接続に落とす。 */
    private const val PING_INTERVAL_MS = 30_000L

    private val client: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(CONNECT_TIMEOUT_MS, TimeUnit.MILLISECONDS)
        .readTimeout(READ_TIMEOUT_MS, TimeUnit.MILLISECONDS)
        .pingInterval(PING_INTERVAL_MS, TimeUnit.MILLISECONDS)
        .build()

    private val nextHandle = AtomicLong(1)
    private val queues = ConcurrentHashMap<Long, LinkedBlockingQueue<String>>()
    private val sockets = ConcurrentHashMap<Long, WebSocket>()

    /**
     * Rust（JNI）用の入口。`url`（`ws://` / `wss://`。OkHttp が http(s) に読み替える）へ WS を
     * 開き、イベント取り出し用のハンドルを返す。接続は非同期で、確立失敗も [EVENT_CLOSED]
     * イベントとして [awaitEvent] に返る（Web の `WebSocket` が onclose を出すのと同型）。
     */
    @JvmStatic
    fun open(url: String): Long {
        val handle = nextHandle.getAndIncrement()
        val queue = LinkedBlockingQueue<String>()
        queues[handle] = queue

        val socket = client.newWebSocket(
            Request.Builder().url(url).build(),
            object : WebSocketListener() {
                override fun onMessage(webSocket: WebSocket, text: String) {
                    queue.put(EVENT_TEXT_PREFIX + text)
                }

                override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                    // サーバ発の close はこちらも閉じて握手を完結させる（onClosed が続く）。
                    webSocket.close(NORMAL_CLOSURE_CODE, null)
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    queue.put(EVENT_CLOSED)
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    // 接続・TLS・I/O の失敗はすべて切断に畳む（Rust 側が backoff 再接続する）。
                    queue.put(EVENT_CLOSED)
                }
            },
        )
        sockets[handle] = socket
        return handle
    }

    /**
     * Rust（JNI）用の同期入口。`handle` の次イベントまで**呼び出しスレッドをブロック**して返す
     * （Rust 側は専用の背景スレッドから呼ぶ契約）。[EVENT_CLOSED] を返したらハンドルは破棄済み。
     */
    @JvmStatic
    fun awaitEvent(handle: Long): String {
        val queue = queues[handle] ?: return EVENT_CLOSED
        val event = queue.take()
        if (event == EVENT_CLOSED) {
            queues.remove(handle)
            sockets.remove(handle)
        }
        return event
    }

    /** `handle` の WS を閉じる。イベント待ちには [EVENT_CLOSED] が流れて解ける。 */
    @JvmStatic
    fun close(handle: Long) {
        sockets[handle]?.close(NORMAL_CLOSURE_CODE, null)
    }
}
