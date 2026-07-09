package com.hayateprojects.hayate.adapter_android_demo

import android.content.Intent
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/**
 * Minimal on-device dev-server URL entry + **Demo selection menu** for Torimi (#534 / #743).
 *
 * Torimi is a framework-agnostic dev-client (Expo Go 相当): the prebuilt native host
 * fetches the App Bundle from a dev-server / Demo Endpoint at runtime rather than reading a
 * baked-in asset. This launcher screen offers two ways to point the host at a bundle:
 *
 *  1. **Demo menu (#743)** — on open, it fetches the public Demo Endpoint's Demo Manifest
 *     (`/demos.json`, ADR-0003) over the OS network stack (`HttpURLConnection` — the same
 *     platform HTTP(S) the native host delegates to, ADR-0002/#740) and lists each demo by
 *     display name. Tapping an entry persists that entry's resolved bundle URL and launches
 *     the native host, which boots exactly that bundle (the entry → boot-target resolution is
 *     host-contract-tested in Rust `demo_manifest`). Demos can be added/renamed by updating the
 *     manifest — no Play review needed (Torimi CONTEXT.md「Demo Manifest」).
 *  2. **URL / QR (#534)** — type a LAN dev-server (`192.168.1.5:5179`) or a full HTTPS bundle
 *     URL, or scan a QR. Unchanged from #534.
 *
 * Either way the target is written to the app's internal files dir; the Rust host reads it back
 * (`dev_server_target`) and uses the one resolved target to drive BOTH the bundle fetch and the
 * reload subscription. Leaving it blank falls back on the native side to the **build default**:
 * the public Demo Endpoint in release (whose manifest's first demo auto-loads — ゼロ入力で動く,
 * #743) or the emulator-loopback dev-server in debug (#534, unchanged).
 *
 * Manifest fetch failure (offline etc.) is non-fatal here: the demo menu is simply omitted and a
 * short note points the user at the URL field — the same「明示エラー＋ URL 入力経路へ誘導」posture
 * the native host takes (ADR-0003). Real "tap a demo → todo boots" is verified on a local device
 * (out of scope for this issue, ADR-0001).
 */
class DevServerSetupActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // ネイティブが読むのと同じファイル（共有 wire 契約名）。前回値を読み戻して field に出す（保持）。
        val urlFile = File(filesDir, DEV_SERVER_URL_FILE)
        val input = EditText(this).apply {
            hint = "dev-server / デモ URL（例 192.168.1.5:5179 や https://…/solid/bundle.js）"
            setSingleLine(true)
            setText(if (urlFile.exists()) urlFile.readText().trim() else "")
        }
        val scan = Button(this).apply { text = "QR スキャン" }
        val connect = Button(this).apply { text = "接続して起動" }

        // デモ選択メニュー（#743）を差し込むコンテナ。マニフェスト取得は非同期で、取れ次第ここに
        // 表示名ボタンを積む。取得失敗時はこのままで、下の URL 入力に誘導する。
        val demoMenu = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
            addView(TextView(this@DevServerSetupActivity).apply { text = "デモを選ぶ、または接続先 URL を入力" })
            addView(demoMenu)
            addView(input)
            addView(scan)
            addView(connect)
        }
        setContentView(layout)

        // 起動先を保存してネイティブ描画へ渡す共通処理（デモ選択／URL 入力／QR で共有）。
        fun persistAndLaunch(target: String) {
            urlFile.writeText(target.trim())
            startActivity(Intent(this, MainActivity::class.java))
            finish()
        }

        // Demo Endpoint の Demo Manifest を OS スタックで取り、表示名ボタンを並べる（#743）。
        loadDemoMenu(demoMenu, ::persistAndLaunch)

        scan.setOnClickListener {
            // 起動コマンドが端末に出した QR（= dev-server の LAN URL）をカメラで読み、欄に入れる。
            // 統一 capability の Android leaf（QrScannerBridge）を共有する。キャンセル/失敗は null。
            QrScannerBridge.startScan(this) { value ->
                val url = value?.trim().orEmpty()
                if (url.isNotEmpty()) input.setText(url)
            }
        }

        connect.setOnClickListener {
            // 入力した URL をネイティブの読み戻し先へ書き、GameActivity（ネイティブ描画）を起動する。
            persistAndLaunch(input.text.toString())
        }
    }

    /**
     * Demo Endpoint（[DEMO_ENDPOINT_URL]）の Demo Manifest（[DEMO_MANIFEST_ROUTE]・ADR-0003）を
     * バックグラウンドで取得し、表示名ごとにボタンを [container] へ積む。タップで、そのエントリの
     * バンドル URL（origin 相対は Demo Endpoint origin に載せて解決）を保存してネイティブを起動する。
     * 取得/解釈に失敗したら（オフライン等）メニューは出さず、URL 入力への誘導だけ残す（謎クラッシュにしない）。
     */
    private fun loadDemoMenu(container: LinearLayout, onSelect: (String) -> Unit) {
        Thread {
            val demos = runCatching { fetchDemoManifest() }.getOrNull()
            runOnUiThread {
                if (demos.isNullOrEmpty()) {
                    container.addView(TextView(this).apply {
                        text = "デモ一覧を取得できませんでした。下に接続先 URL を入力してください。"
                    })
                    return@runOnUiThread
                }
                for ((name, bundleUrl) in demos) {
                    container.addView(Button(this).apply {
                        text = name
                        setOnClickListener { onSelect(bundleUrl) }
                    })
                }
            }
        }.start()
    }

    /**
     * `<Demo Endpoint>/demos.json` を GET してパースし、(表示名, 解決済みバンドル URL) の並びを返す。
     * wire フィールドは `name` / `bundleUrl`（TS `@torimi/dev-server-contract` の値複製）。origin 相対
     * URL は Demo Endpoint origin に載せてフル URL 化する（ネイティブの Direct boot がそのまま fetch する）。
     * **UI スレッドから呼んではならない**（ネットワークは OS スタック＝別スレッド・ADR-0002）。
     */
    private fun fetchDemoManifest(): List<Pair<String, String>> {
        val endpoint = DEMO_ENDPOINT_URL.trimEnd('/')
        val connection = (URL("$endpoint$DEMO_MANIFEST_ROUTE").openConnection() as HttpURLConnection).apply {
            connectTimeout = MANIFEST_FETCH_TIMEOUT_MS
            readTimeout = MANIFEST_FETCH_TIMEOUT_MS
        }
        val body = try {
            if (connection.responseCode != HttpURLConnection.HTTP_OK) return emptyList()
            connection.inputStream.bufferedReader().use { it.readText() }
        } finally {
            connection.disconnect()
        }
        val entries = JSONObject(body).optJSONArray("demos") ?: return emptyList()
        val demos = ArrayList<Pair<String, String>>(entries.length())
        for (i in 0 until entries.length()) {
            val entry = entries.optJSONObject(i) ?: continue
            val name = entry.optString("name").takeIf { it.isNotEmpty() } ?: continue
            val bundleUrl = entry.optString("bundleUrl").takeIf { it.isNotEmpty() } ?: continue
            val resolved = if (bundleUrl.contains("://")) bundleUrl
            else endpoint + if (bundleUrl.startsWith("/")) bundleUrl else "/$bundleUrl"
            demos.add(name to resolved)
        }
        return demos
    }

    companion object {
        /** Rust reader (`dev_server_target::DEV_SERVER_URL_FILE`) と一致させる wire 契約のファイル名。 */
        const val DEV_SERVER_URL_FILE = "torimi-dev-server-url.txt"

        /**
         * 公開 Demo Endpoint（ADR-0003）の URL。Rust の `dev_server_target::DEFAULT_DEMO_ENDPOINT_URL`
         * と一致させる wire 値（ネイティブへ node/Rust 依存を持ち込まないため値で複製する）。実際の
         * workers.dev サブドメイン（account 依存）は配信 account（`pinara`）由来。別 account で
         * ビルドするときは Rust 側 `TORIMI_DEMO_ENDPOINT_URL` と併せてここも差し替える（#743）。
         */
        const val DEMO_ENDPOINT_URL = "https://torimi-demo-endpoint.pinara.workers.dev"

        /** Demo Manifest のルート。Rust `demo_manifest::DEMO_MANIFEST_ROUTE` / TS `demoManifestRoute` と同値。 */
        const val DEMO_MANIFEST_ROUTE = "/demos.json"

        /** Demo Manifest 取得の timeout（ミリ秒）。ネイティブ側 `bundle_source::FETCH_TIMEOUT`（10s）と対称。 */
        const val MANIFEST_FETCH_TIMEOUT_MS = 10_000
    }
}
