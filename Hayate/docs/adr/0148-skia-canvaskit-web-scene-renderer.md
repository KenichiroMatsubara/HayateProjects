# Skia CanvasKit を web の Scene Renderer として導入する（skia-safe の web 対応版）

**Status: proposed**

**Date: 2026-07-12**

## Context

ADR-0146 は skia-safe（rust-skia）による Scene Renderer を**ネイティブ専用**で導入し、
「rust-skia は wasm32-unknown-unknown 非対応のため web / wasm は現行の vello / tiny-skia
構成を不変に保つ」と定めた。結果として:

1. **web だけ Skia 系の描画品質から取り残される。** ネイティブは skia-safe で初の
   `paints_color_glyphs() = true`（COLR/CPAL・ビットマップ絵文字）を得たが、web の CPU
   フォールバック（tiny-skia / vello-cpu）はアウトライングリフのみで絵文字がモノクロに退化する
   （ADR-0101 / ADR-0107）。web と native で描画意味論・品質が乖離する。
2. **「skia-safe を web デモの既定/選択肢に」という要望に応えられない。** 直近で暫定的に
   `skia-safe`（web）を tiny-skia backend へ委譲したが（detect-mode.ts `SKIA_SAFE_WEB_BACKEND`）、
   これは名前だけ skia で実体は tiny-skia の代役にすぎない。

Google Skia には公式の WebAssembly ビルドである **CanvasKit**（`canvaskit-wasm`）が存在する。
これは rust-skia とは別物（C++ Skia を emscripten で wasm 化し JS API を露出したもの）で、
**ブラウザで本物の Skia を動かせる**。CanvasKit は WebGL/WebGPU の GPU 経路と CPU 経路を持ち、
COLR/CPAL・CBDT のカラーグリフを rasterize できる。すなわち CanvasKit は **skia-safe（native）の
web 対応版**として、同じ Skia ファミリで native ↔ web の描画パリティを取る受け皿になる。

本 ADR は「CanvasKit を web の Scene Renderer として導入する」意思決定と、その**アーキテクチャ**
（どの seam に載せ、ADR-0072 の第2 wire 契約棄却をどう維持するか）および**段階導入計画**を記録する。

## Decision

**Skia CanvasKit を web の Scene Renderer として導入する。** 実装は既存の web backend（vello /
tiny-skia / vello-cpu）と**同型の Rust web backend** として行い、`SceneGraph` を歩く `ScenePainter`
実装が CanvasKit の API を wasm-bindgen 経由の JS interop で叩く。CanvasKit は cargo 依存ではなく
**JS/asset 依存**として `hayate-adapter-web` の wasm bundle 初期化時に受け渡す。

### 1. どの seam に載せるか — Rust backend（`SceneGraph` を歩く）

Hayate の retained scene は Rust 内部（`hayate-core::SceneGraph`）にあり、各 backend は
`CanvasBackend::render_scene(&mut self, scene: &SceneGraph, clear_color)` を実装する
（`platform/web/src/backend/*`）。CanvasKit renderer も**この契約を実装する**。scene 歩行と
`ScenePainter` は crate 内部に閉じ（ADR-0054）、CanvasKit 固有の描画呼び出しだけが painter に現れる。

- 新 feature `backend-canvaskit` を `crates/platform/web` に追加し、`backend/canvaskit.rs` に
  `SelectedBackend` を置く（vello / tiny-skia と同じ構造）。
- `ScenePainter` 実装（`SkiaSafe` native の painter とは API が別のため共有しない）が、rect /
  rounded-rect / border / box-shadow / clip / opacity・layer / text glyph / image を CanvasKit の
  `Canvas`・`Paint`・`Path`・`Font`・`Image` 呼び出しへ落とす。
- glyph は Hayate 側で既にシェイプ済み（位置つき）。painter は CanvasKit の `Typeface`/`Font` に
  Hayate の `builtin_fonts` を渡し、位置つきグリフを描く。これにより web で初の
  `paints_color_glyphs() = true` を得る（skia-safe native と同じ勝ち筋）。

### 2. ADR-0072（Raw Layer 外部公開棄却）を維持する

CanvasKit を JS 側の renderer にして「Rust から scene/display-list を JS へ流して replay」する形は、
**第2の公開 wire 契約（Raw Layer 公開）**の復活に等しく ADR-0072 が棄却済み。本 ADR はこれを維持し、
scene は Rust 内部に閉じたまま、CanvasKit interop は backend 内の painter からの下り一方向に留める。
公開サーフェスは Element Layer 1つのまま。

### 3. CanvasKit の初期化と surface

CanvasKit は自前の非同期ローダ（`CanvasKitInit({ locateFile })`）で wasm を読み込む。host bootstrap
（`@torimi/hayate-host` の `loadCanvasBackend`）が canvaskit backend を選んだ場合のみ CanvasKit を
動的 import で読み込み、対象 `<canvas>` 上に GPU（WebGL）または CPU surface を作り、その `Surface`/
`Canvas` ハンドルを Rust backend の `init` に渡す。CanvasKit の wasm（数 MB）は **canvaskit 選択時のみ
lazy load** し、既定バンドルサイズを膨らませない。

### 4. selection 語彙と policy 上の位置

- 値語彙に `canvaskit` を追加する（Rust `SceneRendererKind`、host `resolve-backend.ts`、Tsubame
  `detect-mode.ts`、デモ `index.html` の `#renderer-switch`）。`?renderer=canvaskit` で強制指定できる。
- CanvasKit が web で安定したら、それを **web の主力 Skia レンダラ**に昇格し、暫定委譲
  （`skia-safe` → tiny-skia）を **`skia-safe`/既定 → canvaskit** へ差し替える。tiny-skia は
  「CanvasKit 非対応/低スペック環境向けの CPU フォールバック」に住み分ける。
- ネイティブは skia-safe、web は CanvasKit ——「Skia ファミリで native/web 統一」を Selection Policy
  として明文化する（ADR-0050 の web 側拡張、skia-safe 昇格の別 ADR-0147 と対）。

### 5. スコープ

- **v1:** clear / 単色 fill / rounded-rect / border / box-shadow / clip（overflow）/ opacity・layer /
  text glyph（カラー絵文字含む）/ image。tiny-skia の golden と視覚パリティを合格条件にする。
- **後回し（封印ではない）:** グラデーション・blend mode・フィルタ/シェーダ。encoding と painter
  interface はこれらが契約破壊なしに生えることを設計条件とする（ADR-0141 の姿勢を踏襲）。

## Considered Options

- **A. Rust web backend が CanvasKit を wasm-bindgen 経由で駆動（採用）。** scene 歩行は1本のまま、
  他 backend と同型、ADR-0072 維持。interop の記述量が増えるのが対価。
- **B. JS 側 renderer が Rust から serialize した scene/display-list を replay。** Raw Layer 公開
  （第2 wire 契約）の復活で ADR-0072 に反する。reopen 条件（display list で満たせない実需要）は未達。却下。
- **C. Tsubame `IRenderer` 層に CanvasKit renderer を置く（DomRenderer と同格）。** Canvas Mode では
  レイアウトは Hayate（Rust）が所有するため、この層は computed layout を持たず、独自レイアウトエンジンの
  再実装が要る。却下。

## Consequences

- **描画品質:** web が初めて本物の Skia を得る。カラー絵文字が web でも出る（ADR-0101/0107 の web 側制約を解消）。
- **native/web パリティ:** skia-safe（native）と CanvasKit（web）で同一 Skia の描画結果に収束できる。
- **バンドル:** CanvasKit wasm（数 MB）が増える。canvaskit 選択時のみ lazy load し、既定は据え置き。
  deploy-pages（`.github/workflows/deploy-pages.yml`）に CanvasKit asset の配信を追加する必要がある。
- **build graph:** web の wasm target が1つ増える（`pkg-canvaskit`、feature `backend-canvaskit`）。
  CanvasKit 自体は cargo 依存ではなく npm/asset 依存。
- **検証:** 実描画の検証はブラウザ（Playwright + CanvasKit asset）が要る。selection 語彙などの純ロジックは
  unit test でカバーできるが、painter の視覚パリティは golden/e2e が正準。
- **アクセシビリティ/IME/入力:** host 層（backend 非依存）のため不変（Accessibility Mirror ADR-0124、
  IME ADR-0069、入力自己配線 ADR-0080）。

## Rollout（段階導入）

1. **ADR（本書）＋ selection 語彙の地ならし。** `canvaskit` を語彙に追加し、loader は「未ビルド」を
   明示 throw。デモの切替 UI にはまだ出さない（壊れた選択肢で UX を劣化させない）。純ロジックは unit test。
2. **wasm-pkg target ＋ crate scaffold ＋ CanvasKit init/glue。** `pkg-canvaskit` を manifest に追加、
   `load-canvas-backend.generated.ts` を再生成。CanvasKit を読み込み単色フレームを出すところまで（ブラウザ実証）。
3. **ScenePainter v1。** rect/border/shadow/clip/opacity → tiny-skia golden と視覚パリティ。
4. **text ＋ カラーグリフ。** `builtin_fonts` を CanvasKit へ、位置つきグリフ描画、絵文字カラー化。
5. **layer-present/compositor ＋ 既定昇格。** per-layer 経路を整え、web 既定と `skia-safe` を canvaskit へ
   remap、切替 UI に `canvaskit` ボタンを追加。

> 現環境（web セッション）には wasm32 target / wasm-bindgen が無く Phase 2 以降の wasm ビルド・ブラウザ
> 検証ができない。Phase 2 以降は wasm ツールチェーンの整った環境で実施する。

## 関係

- **ADR-0146（skia-safe をネイティブ専用で導入）:** 本 ADR はその web 対応版。同じ Skia ファミリで
  native/web パリティを取る。「web は vello/tiny-skia 不変」の一節を、CanvasKit という別実装（rust-skia では
  ない）の追加で乗り越える——rust-skia の wasm 非対応という制約自体は不変。
- **ADR-0147（Adreno で vello native を降ろし skia 採用）:** Skia 一本化の native 側決定と対になる web 側決定。
- **ADR-0072（Raw Layer 外部公開棄却）:** **維持**。CanvasKit interop は backend 内部に閉じ、第2 wire 契約を作らない。
- **ADR-0050（Scene Renderer Selection 設計）:** 値語彙・選択理由に `canvaskit` を追加して拡張。
- **ADR-0101 / ADR-0107（カラーグリフの vello 限定・CPU モノクロ退化）:** CanvasKit で web の制約を解消。
- **ADR-0137 / ADR-0138 / ADR-0140（layer-present）:** canvaskit の per-layer 経路は Phase 5 で整合させる。
