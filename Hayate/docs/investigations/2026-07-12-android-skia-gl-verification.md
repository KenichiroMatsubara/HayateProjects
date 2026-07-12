# Android skia GL（Ganesh/EGL）surface の実機検証手順と結果（issue #803）

**Date: 2026-07-12** / **Device: OPPO A101OP（Adreno 620、Android 12）** / **Branch: claude/skia-adoption-800-803**

issue #803（PRD #798 スライス4・ADR-0146 §3）の受け入れ条件「起動確認手順（adb コマンド・確認ログ）の記録」。
品質判断そのもの（既定 surface の確定）は後続の完全人力 issue のスコープで、本書はその実験を再現するための手順と、実装時点の観測結果を残す。

## 切替の口（intent extra）

| キー | 値 | 既定（名前付き定数） |
|---|---|---|
| `hayate.renderer` | `vello` / `skia` | selection policy 既定 = vello 第一候補（`NATIVE_RENDERER_ORDER`、#802） |
| `hayate.skia_surface` | `raster` / `gl` | `DEFAULT_SKIA_SURFACE` = `Gl`（`crates/platform/mobile/android/src/renderer_config.rs`。確定値は後続の完全人力 issue） |

- `hayate.skia_surface` は skia が選ばれたときだけ効く直交軸。GL（EGL）初期化に失敗した端末は
  理由を logcat に残して **skia raster へ一方向 fallback** し、boot は落ちない。
- MainActivity は `android:exported="false"`。**adb は必ず DevServerSetupActivity（唯一の
  exported LAUNCHER）を叩く**——extras はそのまま MainActivity へ転送される（#802 で修正済み、
  `tests/render_selection_switch.rs` が固定）。

## ビルドと導入

```sh
# どちらも安全（Gradle が JNI .so の欠落/stale を検出する）。-Pnativedemo で Hayate 単体デモ。
Hayate/scripts/build-android.sh assembleDebug              # Torimi（Tsubame Todo）変種
Hayate/scripts/build-android.sh assembleDebug -Pnativedemo # ネイティブデモ変種
adb install -r Hayate/crates/platform/mobile/android/android-app/app/build/outputs/apk/debug/app-debug.apk
```

## 起動手順（launcher は画面遷移が要る）

DevServerSetupActivity はデモ選択/URL 入力画面なので、adb だけで完結させる場合はボタンタップまで送る。

```sh
adb logcat -c
# skia GL 強制
adb shell am start -n com.hayateprojects.torimi.debug/com.hayateprojects.hayate.adapter_android_demo.DevServerSetupActivity \
  -e hayate.renderer skia -e hayate.skia_surface gl
# デモメニュー（例 "TODO (SOLID)" ボタン、上から1つ目）をタップして MainActivity へ。
# 座標は `adb shell uiautomator dump` の bounds から取る。この端末では:
adb shell input tap 540 287    # TODO (SOLID)（Torimi 変種）
# nativedemo 変種は「接続して起動」ボタン（URL 空で可）: adb shell input tap 540 855
```

- skia raster 強制: `-e hayate.renderer skia -e hayate.skia_surface raster`
- 既定（vello）確認: extras なしで同じ手順。

## 期待する logcat（実機で観測した実ログ）

skia GL 選択時:

```
HayateSafeArea: renderer override: "skia" skia surface: "gl"
hayate_adapter_android: scene renderer rejected: vello (DisabledByPolicy)
hayate_adapter_android: hayate-adapter-android: skia surface config: gl
hayate_adapter_android: hayate-adapter-android: skia GL surface — EGL vendor=Android EGL version=1.5 Android META-EGL (1.5) GL vendor=Qualcomm GL renderer=Adreno (TM) 620 GL version=OpenGL ES 3.2 V@0502.0 (…) stencil=8
hayate_adapter_android: selected scene renderer: skia
hayate_adapter_android: hayate-adapter-android: skia surface: gl (Ganesh/EGL)
```

skia raster 選択時は `skia surface config: raster` → `skia surface: raster (CPU)`。
extras なしは `renderer override: ""` → `render config — backend=vulkan aa=area` → `selected scene renderer: vello`（既定構成不変）。
GL 初期化失敗時（EGL 不調端末）は `skia GL surface init failed: <理由> — falling back to skia raster` が出て raster で継続する（契約テスト `tests/skia_gl_surface_switch.rs` が文言を固定。今回の実機では EGL が健全なため実発火は未観測）。

## 観測結果（2026-07-12、OPPO A101OP）

- **skia GL で UI が GPU 描画される**: Torimi 変種の Tsubame Todo（日本語テキスト・角丸・シャドウ・
  プログレスバー）が skia GL で描画され、スクロール・タップも動作。`screencap` で確認。
- **painter の surface 非依存の実証**: nativedemo の同一シーンで skia raster と skia GL の
  スクリーンショットが内容一致（scene→Canvas 変換層は無変更のまま、Canvas の出自が
  raster surface → Ganesh FBO0 wrap に替わっただけ）。
- **カラー絵文字**: nativedemo の `🎉😀🚀` が GL でもフルカラーで描画される。この端末は
  Android 12 のため system 絵文字フォントは **CBDT ビットマップ**経路（#802 と同じ）。
  **COLRv1 グラデーショングリフの Ganesh GL 下での評価は本端末では検証できず**（COLRv1 の
  NotoColorEmoji は Android 13+）。skia-safe 0.99.0 CPU raster の既知事象（COLRv1 をフラットに
  落とす）が GL で変わるかの再確認は、COLRv1 フォントを載せられる後続 issue へ持ち越し。
- **既定（vello）不変**: extras なし起動は従来どおり vello（Vulkan + Area）。

### 検証途中で発見・修正した painter の品質バグ（GL 起因ではない）

テキストの多い実シーン（Tsubame Todo）で skia 選択時（**raster / GL 共通**）、起動 15 秒前後で
プロセスが LMK に殺されていた。原因は painter（#800）が TextRun ごと・フレームごとに
`FontMgr::default()`（Android では system font config XML のパース＋全フォント列挙）と
`new_from_data`（フォントバイト列全体の SkData コピー）を作り直していたこと
（`[SkFontMgr Android Parser]` の毎フレームログ洪水とネイティブメモリ膨張で観測）。
`crates/scene-renderers/skia/src/painter.rs` に thread_local の SkFontMgr 共有＋
`Blob::id()` キーの typeface 常駐キャッシュを入れて解消（単体テストで固定）。修正後、
skia GL / raster とも 90 秒＋スクロール操作を経て安定動作。

## 関係

- ADR-0146 §3（surface 非依存 painter・Android GL 計画）、ADR-0147、ADR-0145（intent extra 切替の流儀）
- spec §4 REND-14/15（本 issue で ✅ へ更新）
- 実装: `crates/platform/mobile/android/src/skia_gl_window.rs`（EGL 管理のアダプタ封じ込め、REND-07）、
  `renderer_config.rs`（`SKIA_SURFACE_INTENT_EXTRA` / `DEFAULT_SKIA_SURFACE`）、
  契約テスト `tests/skia_gl_surface_switch.rs`
