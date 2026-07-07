package com.hayateprojects.hayate.adapter_android_demo

import android.app.Activity
import android.content.Context
import android.util.Log
import android.view.Gravity
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.TextView

/**
 * Hayate（要素ツリー→GPU パイプライン）に一切依存しない、素の Android View によるエラー
 * オーバーレイ（#530 系）。
 *
 * Torimi の Web ホスト（`Torimi/host-web` の built-in error panel）は生 DOM/CSS で
 * エラーを描く——Hayate/WebGPU 自体の初期化が失敗しても描画できる「潰れない土台」が要るため。
 * Android には元々それに相当する GPU 非依存の表示手段が無く、boot 失敗・panic を画面に出す
 * 唯一の経路が Hayate 自身の GPU パイプラインだった（= Hayate/GPU init 自体が壊れると何も
 * 出せない）。本クラスはその欠落を埋める：`activity.findViewById(android.R.id.content)`
 * （GameActivity が張るネイティブサーフェスの親）に最前面の `TextView` を足すだけの、
 * Hayate/wgpu を一切経由しない素の Android UI。
 *
 * Rust 側（`error_overlay.rs`）が JNI 経由で直接呼ぶ——`QrScannerBridge` と同じブリッジ
 * パターン（本アプリで唯一の他の Rust↔Kotlin JNI seam）。UI 操作は必ず UI スレッドで行う
 * 必要があるため、呼び出し元スレッドをブロックせず `runOnUiThread` へ投げるだけ（結果を
 * 待たない — boot 失敗ハンドリングや panic hook からも安全に呼べる）。
 */
object ErrorOverlayBridge {
    private const val OVERLAY_TAG = "hayate_error_overlay"
    private const val LOG_TAG = "ErrorOverlayBridge"

    // Web ホストの built-in error panel（`background:#0b1020` / `color:#fca5a5`）と揃えた配色。
    private const val BACKGROUND_COLOR = 0xFF0B1020.toInt()
    private const val TEXT_COLOR = 0xFFFCA5A5.toInt()

    /**
     * 画面いっぱいのオーバーレイに明示エラーメッセージを表示する。boot 失敗ハンドリング・
     * panic hook の両方から呼ばれる想定。既にオーバーレイがあれば本文だけ差し替える
     * （reload 失敗が連続しても View を積み増さない）。
     */
    /**
     * Rust（`ndk_context`）が持つのは Application Context であって Activity ではないため、
     * JNI 入口は `Context` を受け、Activity は [CurrentActivity] レジストリで解決する。
     * Activity が無い（未登録・破棄済み）ときは表示しようがないのでログだけ残す——
     * ここで例外を投げると「エラー表示の失敗」が新たなクラッシュになるため。
     */
    private fun resolveActivity(context: Context): Activity? =
        (context as? Activity) ?: CurrentActivity.get() ?: run {
            Log.e(LOG_TAG, "前面 Activity が未登録のためエラーオーバーレイを表示できません")
            null
        }

    @JvmStatic
    fun showError(context: Context, message: String) {
        val activity = resolveActivity(context) ?: return
        activity.runOnUiThread {
            val content = activity.findViewById<ViewGroup>(android.R.id.content) ?: return@runOnUiThread
            val overlay = content.findViewWithTag<TextView>(OVERLAY_TAG)
                ?: TextView(activity).apply {
                    tag = OVERLAY_TAG
                    setBackgroundColor(BACKGROUND_COLOR)
                    setTextColor(TEXT_COLOR)
                    textSize = 16f
                    gravity = Gravity.CENTER
                    setPadding(48, 48, 48, 48)
                    content.addView(
                        this,
                        FrameLayout.LayoutParams(
                            FrameLayout.LayoutParams.MATCH_PARENT,
                            FrameLayout.LayoutParams.MATCH_PARENT,
                        ),
                    )
                }
            overlay.text = message
            overlay.bringToFront()
        }
    }

    /** オーバーレイを消す（boot 成功時。Web ホストの `clearBuiltinErrorPanel` と対称）。 */
    @JvmStatic
    fun clearError(context: Context) {
        val activity = resolveActivity(context) ?: return
        activity.runOnUiThread {
            val content = activity.findViewById<ViewGroup>(android.R.id.content) ?: return@runOnUiThread
            content.findViewWithTag<TextView>(OVERLAY_TAG)?.let { content.removeView(it) }
        }
    }
}
