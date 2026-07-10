# Android の安全領域を edge-to-edge ＋ WindowInsets の JNI push でアダプタ内完結する（b2）

**Status: accepted**

**Date: 2026-07-10**

## Context

Android ホスト（GameActivity ＋ `hayate-adapter-android`）は、システムバー/ディスプレイカットアウトの安全領域を「WindowInsets を SurfaceView の `setMargins` にして `ANativeWindow` 自体を安全領域サイズに縮める」マージン方式で処理していた。Kotlin（`MainActivity`）が `ViewCompat.setOnApplyWindowInsetsListener` で systemBars + displayCutout を取得してマージンに適用し、SurfaceView が下がった分だけタッチ座標がずれるので、GameActivity がウィンドウ座標で流す `MotionEvent` を Kotlin 側（`offsetLocation`）で SurfaceView 相対へ平行移動していた。Rust 側は `AndroidApp::content_rect()` 由来の `safe_window_dimensions` でレイアウトビューポートを縮めるフォールバックを持っていた。

2026-07-10 の grill セッションで、この方式が特定端末で破綻することが確定した。

- 実機バグ：OPPO Reno5（ColorOS）では正常だが、**Nothing Phone 3a（Android 15 世代）では描画がステータスバーを侵食する**。全ページ共通の症状で、マージン方式（WindowInsets→`setMargins`）が端末依存で不発になっていることを示す。
- `AndroidApp::content_rect()` はフルウィンドウ (0,0,width,height) を返す端末があり（既知・契約テストに明文化済み）、インセットは `onContentRectChanged` ではなく WindowInsets 経由でしか届かない。そのため Rust 側 content_rect フォールバックでも補正できない。
- 「壊れているのはインセットの取得ではなく適用アーキテクチャ」である。取得は既に androidx.core / `WindowInsetsCompat` で正しくできている。外部ライブラリで縦横を取り直す案は問題の所在を取り違えるため棄却した。

対策として 2 案を検討した。

- **b1（core に安全領域を一次概念として導入）**: `env(safe-area-inset-*)` 相当を Hayate CSS / core に入れる。バグ修正に新スタイル機能を混ぜることになり、Protocol Contract・Tsubame・全レンダラーに波及する。バグ 1 件に対して過大。却下。
- **b2（edge-to-edge ＋ インセットをアダプタ内で処理）**: SurfaceView をフルウィンドウ（edge-to-edge）のままにし、WindowInsets を JNI で Rust へ push して、レイアウトビューポート縮小・シーン平行移動・タッチ座標補正を **アダプタ（`hayate-adapter-android`）内で完結**する。core / Protocol Contract / Tsubame は不変。

## Decision

**b2 を採用する。** 安全領域処理を「ANativeWindow を縮めるマージン方式」から「edge-to-edge ＋ インセットをアダプタ内で処理する方式」へ置き換える。

- **Kotlin（`MainActivity`）**: `setMargins` によるマージン縮小と、Kotlin 側タッチ補正（`MotionEvent.offsetLocation`）を撤去。`WindowCompat.setDecorFitsSystemWindows(window, false)` で SurfaceView をフルウィンドウ（edge-to-edge）に広げる。WindowInsets（`systemBars() or displayCutout()`。IME＝`Type.ime()` は含めない — GameTextInput が別途処理する）を JNI native 関数 `nativePushSafeAreaInsets` で Rust へ push する。リスナー発火ごとに加え、リスナー不発端末（Nothing Phone 3a 実例）への保険として `content.post { rootWindowInsets }` スナップショットも一度 push する。受信値は logcat（タグ `HayateSafeArea`）に記録し、端末別のインセット配送問題を診断可能にする。インセットは消費しない（下流の GameTextInput の IME インセット処理へ流す）。ステータスバーのアイコン色は `isAppearanceLightStatusBars` を名前付き定数 `LIGHT_STATUS_BAR_ICONS` で静的に設定する（アプリテーマからの動的導出は将来の別 issue）。
- **JNI（`jni_bridge.rs`）**: Kotlin→Rust のエクスポート `Java_..._nativePushSafeAreaInsets` を、JNI 封じ込め方針（`qr_scanner_encapsulation.rs`）に従い `jni::` を直接使える唯一のファイルに置く。受け取った物理px インセットを `safe_area::store_pushed_insets` でフレームループ可読なグローバル（atomic）へ格納する。
- **Rust（`safe_area.rs`・アダプタ内で完結）**: 純 Rust シームとして「インセット → レイアウトビューポート縮小（`layout_viewport`）／シーン平行移動原点（`scene_origin`）／タッチ座標補正（`correct_touch`）」を持ち、ホストで単体テストする。GPU surface / swapchain はフルウィンドウサイズのまま。描画時に `VelloSceneRenderer::render_scene_with_offset` でシーンを左/上インセット分だけ右下へ平行移動し、バー裏領域は vello が `base_color`（ルート背景色）でターゲット全面をクリアするのでそのまま塗られる。タッチ座標はインセット分を差し引いてから hit-test に渡す。`content_rect()` 由来の補正（`safe_window_dimensions` / `viewport_for_surface`）は JNI push 値優先のフォールバック（`effective_insets`）に降格し、live 経路からは外れた（回帰テストのために関数自体は残す）。
- **契約テスト**: マージン適用の不在（`setMargins` 無し）・Kotlin 側タッチ補正の不在（`offsetLocation` 無し）・edge-to-edge 有効化・JNI push（`nativePushSafeAreaInsets`）と `rootWindowInsets` スナップショットの存在・IME を含めないこと・インセット非消費・logcat 記録・`isAppearanceLightStatusBars` の名前付き定数化・アダプタ内完結（`render_scene_with_offset` / `scene_origin` / `correct_touch` / `pushed_insets`）を、ソース走査で固定する（Gradle/AGP・wgpu はサンドボックス実行不可のため `apk_packaging.rs` と同方式）。

## Considered Options

- **マージン方式を維持（現状）**: Nothing Phone 3a でリスナーが端末依存で不発になりステータスバー侵食を起こす。`content_rect()` フォールバックも同端末では効かない。却下。
- **b1（core に安全領域を一次概念として導入）**: `env(safe-area-inset-*)` 相当を core / Hayate CSS に入れる。core・Protocol Contract・Tsubame・全レンダラーへ波及し、バグ 1 件の修正としては過大。バグ修正に新スタイル機能を混ぜない方針から却下。
- **外部ライブラリでインセットを取り直す**: 取得は既に `WindowInsetsCompat` で正しくできており、壊れているのは適用アーキテクチャ。問題の所在を取り違えるため却下。
- **b2（採用）**: アダプタ 1 crate ＋ Kotlin 1 ファイルに閉じ、core / Protocol Contract / Tsubame 不変。純粋計算はホストで単体テスト、device 配線はソース契約テストで固定でき、インセットが正しく届く端末（OPPO Reno5 実測相当）では従来と同一の見た目になる。

## Consequences

- `hayate-adapter-android` に純 Rust シーム `safe_area`（`SafeAreaInsets` ＋ push 値グローバル ＋ 3 つの純粋計算）が入り、ホストで単体テストされる。`app.rs` / `app_tsubame.rs`（device 経路）は `effective_insets`（JNI push 優先・content_rect フォールバック）でビューポートを縮め、`render_scene_with_offset` でシーンを平行移動し、`process_touch_input` で `correct_touch` する。安全領域処理が **タッチ・描画・レイアウトで単一のインセット源**に揃う。
- `VelloSceneRenderer` に `render_scene_with_offset`（2D 平行移動）を追加し、`render_scene`（offset 0）と `render_scene_at`（scroll band の `(0, -origin_y)`）をこの一般化経路に集約した。web/desktop/iOS の挙動は不変（既存呼び出しは `render_scene` のまま）。
- マージン方式の設計意図（現行方式の根拠は契約テストのコメントにしかなかった）は本 ADR に引き継いだ。`safe_window_dimensions` / `viewport_for_surface` は content_rect 実測の回帰テストのために残すが、live 経路からは外れた（`#[allow(dead_code)]` ＋ 注記）。
- `isAppearanceLightStatusBars` はアプリテーマから動的導出せず静的固定。テーマ連動は将来の別 issue。
- 実機での最終確認（Nothing Phone 3a でステータスバー侵食が消えること・OPPO Reno5 で見た目不変）は Gradle/実機を要するため、本 ADR のコード変更後に人手で行う。

## 関係

- **refines** ADR-0094（Android packaging: GameActivity ＋ Gradle。GameActivity がタッチをウィンドウ座標で native に流す前提と、Kotlin の薄い glue 方針を引き継ぐ）、ADR-0117（surface lifecycle / capability leaf。物理サーフェス寸法 → 論理ビューポート導出を共有する）。
- **references** ADR-0125（Rust↔Kotlin JNI 封じ込め方針。`nativePushSafeAreaInsets` を `jni_bridge.rs` に置く根拠）、ADR-0127（`render_scene_at` の scroll band 平行移動。本 ADR の `render_scene_with_offset` が一般化して集約した）。
- 動機となった議論: 2026-07-10 grill セッション（Nothing Phone 3a のステータスバー侵食）、issue #794。
