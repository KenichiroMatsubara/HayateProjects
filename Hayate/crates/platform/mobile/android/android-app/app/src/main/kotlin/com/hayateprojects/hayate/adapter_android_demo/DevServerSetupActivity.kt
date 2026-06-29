package com.hayateprojects.hayate.adapter_android_demo

import android.content.Intent
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import java.io.File

/**
 * Minimal on-device dev-server URL entry screen for Miharashi (#534).
 *
 * Miharashi is a framework-agnostic dev-client (Expo Go 相当): the prebuilt native host
 * fetches the App Bundle from a dev-server at runtime rather than reading a baked-in asset.
 * This launcher screen lets the user type the dev-server URL on the device itself — e.g.
 * `192.168.1.5:5179` or `http://192.168.1.5:5179` — and persists it to the app's internal
 * files dir. The Rust host reads it back (`dev_server_target`) and uses the one resolved
 * target to drive BOTH the bundle fetch (HTTP) and the reload subscription (WS); leaving it
 * blank falls back to the emulator-loopback default on the native side.
 *
 * Deliberately no networking / parsing here — the field's last value is shown again on the
 * next launch (retention), and the URL handling (parse / retention / reconnection) is
 * host-contract-tested in Rust (`dev_server_target`). Real "type a URL → todo boots" is
 * verified on a local device (out of scope for this issue).
 *
 * QR scanning: the dev-server startup command prints the LAN URL as a QR code, so on a real
 * device the user can tap "QR スキャン" to read it with the camera instead of typing the IP.
 * This uses Google Code Scanner (Play services scanner UI) — no CameraX, no camera permission
 * to declare; the scanned `rawValue` (= the dev-server URL) just fills the field. The actual
 * URL handling stays unchanged (write to the shared file → launch GameActivity), so the Rust
 * host contract is untouched.
 */
class DevServerSetupActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // ネイティブが読むのと同じファイル（共有 wire 契約名）。前回値を読み戻して field に出す（保持）。
        val urlFile = File(filesDir, DEV_SERVER_URL_FILE)
        val input = EditText(this).apply {
            hint = "dev-server URL（例 192.168.1.5:5179）"
            setSingleLine(true)
            setText(if (urlFile.exists()) urlFile.readText().trim() else "")
        }
        val scan = Button(this).apply { text = "QR スキャン" }
        val connect = Button(this).apply { text = "接続して起動" }

        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
            addView(input)
            addView(scan)
            addView(connect)
        }
        setContentView(layout)

        scan.setOnClickListener {
            // 起動コマンドが端末に出した QR（= dev-server の LAN URL）をカメラで読み、欄に入れる。
            // Google Code Scanner は UI もカメラ取得も Play services 側が担うので、ここは結果を
            // 受け取って EditText に流すだけ（接続フローは「接続して起動」と同じく手入力を経由する）。
            val options = GmsBarcodeScannerOptions.Builder()
                .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
                .build()
            GmsBarcodeScanning.getClient(this, options)
                .startScan()
                .addOnSuccessListener { barcode ->
                    val value = barcode.rawValue?.trim().orEmpty()
                    if (value.isNotEmpty()) input.setText(value)
                }
                .addOnCanceledListener {
                    // ユーザーがスキャンを閉じた：何もしない（手入力に戻れる）。
                }
                .addOnFailureListener { error ->
                    Toast.makeText(
                        this,
                        "QR スキャンを使えませんでした: ${error.message}",
                        Toast.LENGTH_LONG,
                    ).show()
                }
        }

        connect.setOnClickListener {
            // 入力した URL をネイティブの読み戻し先へ書き、GameActivity（ネイティブ描画）を起動する。
            urlFile.writeText(input.text.toString().trim())
            startActivity(Intent(this, MainActivity::class.java))
            finish()
        }
    }

    companion object {
        /** Rust reader (`dev_server_target::DEV_SERVER_URL_FILE`) と一致させる wire 契約のファイル名。 */
        const val DEV_SERVER_URL_FILE = "miharashi-dev-server-url.txt"
    }
}
