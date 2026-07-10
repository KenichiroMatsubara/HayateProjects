# Android の vello AA 方式と wgpu バックエンドをランタイム切替可能にする（Adreno 710 切り分けスイッチ）

**Status: accepted**

**Date: 2026-07-10**

## Context

Android ホストの vello 描画経路で、**Nothing Phone 3a（Adreno 710）で CSS Gallery ページのみパス描画が破綻する**（角丸カード・シャドウがギザギザの多角形の塊になる）。単純な Todo ページは正常。OPPO Reno5（Adreno 618）では全ページ正常。2026-07-10 の grill セッションで切り分けを進めた結果、以下が判明した。

- **同じ Nothing Phone 3a の Chrome（WebGPU / Dawn）では vello が正常に描画する**（検証済み）。同一 GPU・同一 vello アルゴリズム（Area AA 含む）が Dawn 経由なら正しく動くため、シーン構築側と vello 本体はシロ。**容疑は wgpu-native の Vulkan 経路（Naga 生成 SPIR-V ＋ Dawn が内蔵する Qualcomm 向けドライバ回避策の不在）× Adreno 710 ドライバ**に絞り込み済み。
- 現行コードは AA を Area 固定（`AaSupport::area_only()`）、バックエンドを `Backends::VULKAN` 固定にしている。Area AA はコンピュートシェーダの atomics に最も依存する経路で、「複雑なシーンでだけ破綻」という症状と整合する。
- 恒久対策（MSAA 既定化 or バックエンド切替 or デバイス別分岐）は実機実験の結果で決める。本 issue はその実験を**再ビルド不要**で回せるようにするスイッチの実装で、実験自体は後続の完全人力 issue。

## Decision

ADR-0138/0140 が確立した「**常時コンパイル＋ランタイムフラグ**」流儀（cargo feature や別ビルドを作らない）に従い、AA 方式と wgpu バックエンドをランタイム注入可能にする。

- **AA 方式（`hayate-scene-renderer-vello`）**: `VelloAaMethod`（`Area` / `Msaa8` / `Msaa16`）と既定定数 `DEFAULT_AA_METHOD = Area` を追加。`VelloSceneRenderer::new_with_options(device, cache, aa)` で注入し、選んだ方式のパイプラインだけをコンパイル（`AaSupport: FromIterator<AaConfig>` で該当フラグのみ）した上で、warmup / render の `RenderParams::antialiasing_method` も同じ config で回す（support と config を必ず一致させる）。`new` / `new_with_pipeline_cache` は既定 Area へ委譲するので、**web / desktop / iOS は挙動不変**。Area AA を default に据えるのは現行維持のためで、後続の実験がこの定数を確定させる。
- **wgpu バックエンド（`hayate-adapter-android`）**: 純 Rust シーム `render_config` に `WgpuBackend`（`Vulkan` / `Gl`）と既定定数 `DEFAULT_WGPU_BACKEND = Vulkan` を追加。`init_gpu_surface` の `wgpu::Instance` 生成で `effective_backend().to_wgpu()` を使う（`Backends::VULKAN` 固定を廃止）。
- **実行時上書き（intent extra）**: `adb shell am start -e hayate.backend gl -e hayate.aa msaa8` で APK を作り直さずに 3 実験（MSAA8/16・GL）を回せる。`MainActivity.onCreate` が `intent.getStringExtra` で読み（未指定は空文字）、JNI native 関数 `nativePushRenderConfig` で Rust へ push する。`jni_bridge.rs`（JNI 封じ込め方針）が受けて `render_config::store_pushed_config` へ渡し、`resolve_backend` / `resolve_aa` が未指定/未知値を既定へ落とす。既定値は名前付き定数（マジック値の禁止）。
- **logcat 記録**: 選択された AA 方式・バックエンドと、GPU アダプタ情報（`adapter.get_info()` の名前・バックエンド・ドライバ・ドライバ情報）を logcat に出す。実験記録と上流報告（wgpu/naga）にそのまま使えるようにする。
- **GL 取得失敗の耐性**: `init_gpu_surface` は Result を返し、adapter/device 取得失敗（GL 非対応端末を含む）は `Err` に理由を残す。`CreateSurface` ハンドラはそれを logcat に出して surface 無しで続行するので、GL 選択で取得に失敗しても boot は落ちない（既存の GPU init 失敗ハンドリングと同経路）。
- **契約テスト**: バックエンドのランタイム選択（`effective_backend().to_wgpu()`、`Backends::VULKAN` 固定の不在）・AA 注入（`new_with_options` + `effective_aa`）・web が既定 Area のまま（`VelloSceneRenderer::new`、`new_with_options` 不使用）・intent extra（`hayate.backend` / `hayate.aa`）と JNI push（`nativePushRenderConfig` → `store_pushed_config`）・logcat 記録（`get_info` + `log::info!`）・GL 失敗で boot が落ちないこと（`GPU init failed` ログ）を、ソース走査で固定する（Gradle/AGP・wgpu はサンドボックス実行不可のため `ios_packaging.rs` と同方式）。純粋部（enum 解釈・resolve・既定値・グローバル格納）はホストで単体テストする。

## Considered Options

- **cargo feature / 別ビルドで AA・バックエンドを切る**: ADR-0138/0140 が廃した 2 バイナリ構成の再来で、保守コスト（別 pkg・ビルドスクリプト・CI エントリ）を積む。実機実験は 1 台で複数条件を素早く回したいので再ビルド前提は不向き。却下。
- **いきなり MSAA を既定化する / バックエンドを GL 固定にする**: 恒久対策を実機データ無しに決め打ちすることになる。MSAA は帯域・電力コストがあり、GL は wgpu の Vulkan 専用機能（パイプラインキャッシュ ADR-0130b 等）を失う。まず切り分けスイッチを入れて実験してから決める。却下（本 issue のスコープ外）。
- **デバイス別分岐（Adreno 710 を検出して自動でフォールバック）**: 恒久対策の候補の一つだが、どのフォールバックが正しいかは実験前で不明。まずスイッチ、分岐は実験後。却下（後続 issue）。
- **常時コンパイル＋ランタイムフラグ（intent extra）（採用）**: 1 台で再ビルドなしに Area/MSAA8/MSAA16 × Vulkan/GL を切り替えられ、web/desktop/iOS は既定で挙動不変。ADR-0138/0140 と同じ流儀で保守コストを増やさない。

## Consequences

- `hayate-scene-renderer-vello` に `VelloAaMethod` / `DEFAULT_AA_METHOD` / `new_with_options` が入り、`VelloSceneRenderer` が AA 方式を保持する。`new` / `new_with_pipeline_cache` は既定 Area へ委譲（web/desktop/iOS 不変）。AA 方式の parse/既定はホストで単体テストされる。
- `hayate-adapter-android` に純 Rust シーム `render_config`（`WgpuBackend` / 既定定数 / `resolve_*` / intent extra グローバル格納）が入り、ホストで単体テストされる。`init_gpu_surface` はバックエンドをランタイム選択し、AA を注入し、選択とアダプタ情報を logcat に出す。
- 実行時上書きの口は intent extra（`hayate.backend` / `hayate.aa`）で、`MainActivity` → `nativePushRenderConfig` → `render_config::store_pushed_config` の Kotlin→Rust JNI push（`nativePushSafeAreaInsets` と同じ着地パターン）。
- 既定値（Area・Vulkan）は名前付き定数で据え置き、**恒久対策は本スイッチを使った実機実験の結果で後続の完全人力 issue が確定させる**。その issue が `DEFAULT_AA_METHOD` / `DEFAULT_WGPU_BACKEND`、あるいはデバイス別分岐を決める。
- GL 選択で adapter/device 取得に失敗する端末でも boot は落ちず、失敗理由が logcat に残る（既存の GPU init 失敗ハンドリングを流用）。

## 関係

- **references** ADR-0138（tiny-skia/vello_cpu が先行して確立した「常時コンパイル＋ランタイムフラグ」パターン）、ADR-0140（vello layer-present のランタイムフラグ化。cargo feature を廃してランタイム既定値＋コメントで運用する流儀の直近適用例）、ADR-0130b（Vulkan 専用のパイプラインキャッシュ。GL 選択時は使えないことの背景）。
- 動機となった議論: 2026-07-10 grill セッション（Nothing Phone 3a / Adreno 710 の CSS Gallery パス描画破綻の切り分け）、issue #795。恒久対策は後続の完全人力 issue。
