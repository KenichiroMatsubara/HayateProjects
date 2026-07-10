# skia-safe Scene Renderer をネイティブ専用で導入する（レンダラ戦略の転換）

**Status: accepted**

**Date: 2026-07-10**

## Context

二つの独立した問題が同じ答えを指している。

1. **Adreno 破綻の保険が無い。** Nothing Phone 3a（Adreno 710）で vello/wgpu の Vulkan 経路がパス描画を破綻させる（issue #795、切り分けスイッチは ADR-0145）。同一端末の Chrome（WebGPU / Dawn）では vello が正常描画するため、容疑は wgpu-native の Vulkan 経路 × Adreno ドライバに絞られているが、**wgpu/naga 上流での恒久解決は保証がない**。切り分け実験（issue #796）がどの構成でも直らない場合、Android を乗り換えられる主力候補レンダラが現在存在しない。vello をネイティブビルドから外す将来すらあり得るため、その受け皿が要る。
2. **CPU 描画の品質上限が低い。** 現行の CPU 系 Scene Renderer（tiny-skia / vello-cpu）はアウトライングリフしか描けず、カラー絵文字がモノクロに退化する（ADR-0101 がフォールバックをモノクロ Noto Emoji に固定、ADR-0107 がカラーを Vello 限定と決定）。GPU が使えない環境での描画品質の上限を上げる手段がない。

issue #796 のエスカレーション節は「skia-safe 導入はバグ修正ではなくレンダラ戦略の転換＝ADR 必須の意思決定」と定め、2026-07-10 の grill セッションを経て PRD #798 が決定一式を確定した。本 ADR はその決定の正式な記録である。

Google Skia は Android HWUI / Chrome の実レンダラでありドライバ成熟度が最高（Adreno 向け workaround を内蔵）、かつ scaler が COLR/CPAL・ビットマップ絵文字を処理する。Rust バインディング skia-safe（rust-skia）は wasm32-unknown-unknown 非対応のため、導入はネイティブ専用となる。

## Decision

**skia-safe による Scene Renderer をネイティブ専用（desktop + Android）で導入する。** iOS は対象外（別 issue）。web / wasm は現行の vello / tiny-skia 構成を不変に保つ。

### 1. 動機の序列

主目的は**描画品質・機能の上限向上**（カラーグリフ等、Vello 以外で初の `paints_color_glyphs()` = true レンダラ）。実質的な引き金は #795/#796 の Adreno 破綻に対する**保険**で、「vello をネイティブに含めない」将来の受け皿を兼ねる。

### 2. Renderer Selection Policy 上の位置

- skia はネイティブの **standard alternative**。既定順序は **vello → skia の一方向 fallback**（GPU が生きている環境で vello を降ろす理由はまだない）。
- tiny-skia は **web 専用 CPU フォールバック**へ住み分ける（ネイティブに結線しない。spec §4 REND-11 を更新）。
- skia を preferred default に昇格するかは**実測後の別 ADR**。

### 3. surface 戦略 — painter は surface 非依存、raster → Android GL（EGL）

- `scene-renderers/skia` crate の painter は **surface 非依存**（渡された Skia Canvas に描くだけ）に作る。walk・planning は core/compositor の共有実装のまま（REND-04/05 維持）。
- 導入は **CPU raster** で desktop → Android の順に結線する。desktop は型作り・golden の場、Android が本命。
- その後**早期フォローアップ**（「将来の保険」ではなく計画内）として **Android の Skia GL（Ganesh / EGL）** surface を実装する。HWUI/Chrome が長年叩いた GL 経路を取る。#795 の wgpu GL スイッチで得る EGL 知見を流用する。
- EGL コンテキスト管理は Android platform adapter に閉じ、core に第二の GPU 抽象を持ち込まない（REND-07 維持）。GL 昇格は adapter 側の工事だけで済む。

### 4. テキスト — レイアウト正本は parley、Skia はグリフラスタのみ

- レイアウト正本は **parley のまま**。TextRun の確定済みグリフ ID＋位置を **SkTextBlob** にして描画する。SkParagraph / SkShaper は使わない（レイアウト・hit-test・IFC 意味論がレンダラ間で一致し続ける）。
- SkTypeface は fontique が読んだ**同一フォントバイト列**から生成する（グリフ ID の一致が前提。崩れたら golden で豆腐/別字として検出）。
- `paints_color_glyphs()` は **true**。フォント調達は既存シーム（ADR-0107 の renderer dispatch ＋ ADR-0101 の `upgrades_to_color_emoji`）を参照してカラー版フォントを選ぶ。

### 5. リンク構成 — 両レンダラ併載＋ランタイム切替＋ `backend-vello` feature

- vello と skia は**両方リンクしランタイム切替**（ADR-0138/0140/0145 の「常時コンパイル＋ランタイムフラグ」流儀）。上書きの口は Android: intent extra / desktop: env・CLI フラグ。
- 選択されたレンダラ・選択理由（`RendererSelectionReason`）・GL 時は EGL/GPU 情報を logcat / stderr に記録する（web と同じ観測可能な語彙で採否を追う）。
- 将来 vello をネイティブから外せる **`backend-vello` feature（default on）** を出口として用意する（確定時に Skia ~10MB 級併載分のバイナリサイズを相殺）。
- web の「1バイナリ1レンダラ」排他（REND-11 の二 WASM バイナリ方式）は**ネイティブでは採らない**。

### 6. レイヤコンポジタ

既存の `LayerRasterizer` / `LayerCompositor` trait（ADR-0125、backend 非依存）を実装する。キャッシュ面 = SkSurface、合成 = drawImage。planning（PresentPlanner）は共有のまま。tiny-skia 実装が雛形。

### 7. ビルド供給 — ソースベンダリングしない（ADR-0007 追記）

crates.io の skia-safe＋ビルド済みバイナリを使い、版は**厳密ピン**、CI はバイナリをキャッシュする。ソースベンダリング・fork はしない — Skia に手を入れる意図がなく、**Google の実績をそのまま使うのが動機**だからである。ADR-0007 に「wgpu 同類の例外」として追記する。GitHub Releases のバイナリ取得が問題を起こした場合の hardening として `SKIA_BINARIES_URL` 系のセルフホストミラーへ切り替えられる（記録のみ、既定では使わない）。skia-safe バイナリ取得込みのクリーンビルドが CI で通ることをスライス1の受け入れ条件に含める（ビルド時ネットワーク依存の検知）。

### 8. テスト

- **per-renderer golden 方式**（既存3レンダラと同じ流儀）: skia crate 専用の golden PNG で自分の過去出力との回帰のみ固定。レンダラ間のピクセル比較はしない（AA・ガンマ・ヒンティングが異なり原理的に不一致）。シーンの同一性は共有 fixtures（demo-fixtures）と DrawOp レベル（RecordingPainter）で担保。
- レイヤ分解の正しさは tiny-skia の `tests/layer_compositor.rs` と同型のピクセルパリティテスト（skia 内での raster-直描き一致）で固定。
- **CI 対象は desktop raster 経路のみ**（Linux で走る）。Android GL は実機が要るため CI golden の対象外とし、#796 同様の完全人力・実機確認 issue で担保。

### 9. 前提工事

ネイティブには selection policy ループが存在しない（desktop は vello 直結）。desktop への Render Host 芯導入（REND-08 / ADR-0068 / ADR-0132 の hoist 継続）が結線スライスの前提工事となる。**シームはすべて既存**（`ScenePainter` / `SceneRenderer::render_scene` / `LayerRasterizer`・`LayerCompositor` / `SceneRendererKind`＋Renderer Selection Policy / `Surface` trait）で、新シームは作らない。唯一の追加工事は「既存 policy シームをネイティブに通す」こと。

## Considered Options

- **Skia Vulkan バックエンド**: 容疑者が Adreno の Vulkan ドライバである以上、Vulkan 経路を取れば保険の意味が消える。却下。
- **Graphite（Skia の次世代バックエンド）**: 成熟度・実績の論拠が弱い（本 ADR の採用理由は「HWUI/Chrome が長年叩いた実績」であり、Graphite にはそれが無い）。却下。
- **SkParagraph / SkShaper によるテキストレイアウト**: レイアウト正本（parley）の二重化になり、hit-test・IME・IFC 意味論がレンダラ間で分岐する。却下。
- **desktop の GPU（GL）経路**: desktop に描画破綻の問題が実在しない。desktop は raster のまま。却下。
- **web の「1バイナリ1レンダラ」排他をネイティブにも適用**: 実機 A/B 実験（#796 流儀）と runtime fallback の双方が再ビルド無しの切替を要求する。ADR-0138/0140/0145 の流儀に従い両載せ＋ランタイム切替とする。却下。
- **Skia ソースのベンダリング / fork**: 巨大 C++ ツリーで保守コストが利益を上回り、そもそも手を入れる意図が無い。却下（ADR-0007 追記）。
- **tiny-skia をネイティブ CPU フォールバックとして維持**: アウトライングリフしか描けず品質上限が低い（Context 2）。skia がネイティブの代替経路を担い、tiny-skia は web 専用へ住み分け。却下。

## Consequences

- `scene-renderers/skia` crate が新設され、`ScenePainter` と `LayerRasterizer`/`LayerCompositor` の実装＋per-renderer golden だけを持つ（walk・planning は共有のまま重複しない）。
- ネイティブバイナリは vello/wgpu と Skia（~10MB 級）を併載する。`backend-vello` feature（default on）が戦略転換確定時の相殺手段。
- ネイティブに Renderer Selection Policy が通り、REND-08（Render Host 芯 hoist）が前進して web 専用実装が解消に向かう。
- ADR-0007 に skia-safe の例外（wgpu 同類・厳密ピン・ミラー hardening）が追記される。
- spec §4 に REND-14（skia Scene Renderer）/ REND-15（ネイティブ selection policy）が ⬜ で追加され、REND-11 が web 専用の住み分けへ更新される。実装スライス（crate 新設 → desktop 結線 → Android raster → Android GL）は PRD #798 の子 issue が ✅ へ進める。
- wasm32 は対象外（skia-safe 非対応）。web の挙動・構成は不変。
- vello-cpu スパイクの去就、カラー絵文字フォントの調達方針の変更、iOS 対応、skia の preferred default 昇格は本 ADR のスコープ外（それぞれ独立の検証系・既存シーム・別 issue・実測後の別 ADR）。

## 関係

- **動機となった議論**: PRD #798（2026-07-10 grill セッションの決定一式）、issue #795（Adreno 710 破綻の切り分けスイッチ）、issue #796（実機実験とエスカレーション節「skia-safe 導入は ADR 必須のレンダラ戦略転換」）。
- **amends** ADR-0007（vendored dependencies — skia-safe を wgpu 同類の「ベンダリング対象外」例外として追記）。
- **references** ADR-0050（Backend / Scene Renderer 分離と Renderer Selection Policy）、ADR-0068・ADR-0132（Render Host 芯の共有層 hoist — ネイティブ結線の前提工事）、ADR-0101・ADR-0107（カラーグリフの `paints_color_glyphs()` シームと font_coverage の格上げ判定）、ADR-0125（LayerRasterizer / LayerCompositor trait）、ADR-0138・ADR-0140・ADR-0145（「常時コンパイル＋ランタイムフラグ」流儀）。
