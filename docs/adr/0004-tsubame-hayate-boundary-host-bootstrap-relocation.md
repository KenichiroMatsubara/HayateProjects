# Tsubame と Hayate の境界を引き直し、host bootstrap を Tsubame から退去させる

> **用語更新（Tsubame ADR-0011・2026-06-27）**: 本 ADR の "Canvas Renderer" / `CanvasRenderer` / `@tsubame/renderer-canvas` は **Hayate Renderer** / `HayateRenderer` / `@torimi/tsubame-renderer-hayate` に改名された。本文は決定当時の記録として原文のまま。

**Status: proposed**

**Date: 2026-06-24**

## Context

「Tsubame が canvas を作り出してそこに描画する」という機構が、Android などネイティブ
（ホストが Surface・vsync・IME を所有し、Hayate が cdylib として描く）と乖離している、
という問題提起から出発した。掘り下げると真因は別だった。

`@tsubame/renderer-canvas` パッケージが **host bootstrap を吸い込んでいる**:

- `src/init.ts` が Hayate の WASM（`hayate-adapter-web`, 実体は `Hayate/wasm-pkgs/pkg`）を
  動的 import し、`navigator.gpu` をプローブし、`HayateElementRenderer.init(canvas)` で
  canvas を取得する。Tsubame の renderer パッケージが Hayate ランタイムと DOM canvas の
  両方を直接知っている。
- `src/init-android.ts` が native host glue（注入された `RawHayate` の結線、native vsync
  ポンプ）を持つ。
- `src/android.ts`（`/android` サブパス）は、index 経由だと `init.ts` が WASM を巻き込んで
  IIFE バンドルへ巨大 base64 が混入するのを避けるためだけに存在する。これは原因ではなく
  **症状**。

結果、`CanvasRenderer` 本体にも `canvas?: HTMLCanvasElement` / `ResizeObserver` /
`syncEditContext` / `devicePixelRatio` / `requestAnimationFrame` 既定が染み込み、Android は
それらを `canvas: null` で**無効化**することで成立していた。ネイティブは host を
「知らない」のではなく「知識を消して」動いていた。どのファイルが Tsubame のものか Hayate
のものか不明瞭なまま、層の議論ができない状態だった。

## Decision

### 1. Host 結合原則

Tsubame の renderer は host を *掴みに行かない* — 注入されたハンドルとして *受け取る*。

- **Canvas Renderer（可搬経路・強形）**: host は描画先と分離する（Hayate が surface を
  描く）。renderer は frame-clock の tick だけを受け取り、surface・resize・pointer・IME は
  host 側 adapter が所有する。renderer は platform 識別子をゼロ保持（`HTMLCanvasElement`
  型も canvas 参照も持たない）。
- **DOM Renderer（公言された browser 結合経路・弱形）**: host == 描画先（DOM）。
  `container` / `doc` を注入で受け取る。可搬性を主張しないので DOM 結合でよい。

### 2. 3 者境界と依存方向

| 所属 | 持つもの |
| --- | --- |
| **Tsubame** | `renderer-protocol` / `renderer-dom` / `renderer-canvas` core（`CanvasRenderer({raw, clock})` + packet/encoder + protocol 再 export）/ `solid` |
| **Hayate** | `hayate-adapter-web`（WASM, RawHayate 提供・pointer/resize/IME 自己配線）+ web host bootstrap / native cdylib + native host bootstrap |
| **App / demo** | `main.tsx` / `main.android.tsx`（host から `raw`(+clock) を得 → `new CanvasRenderer({raw, clock})` → `renderTsubame(App, r)`） |

- **Hayate → Tsubame: 依存なし（永久, ADR-0001 維持）。**
- **Tsubame → Hayate: Contract 経由のみ。** `@torimi/hayate-protocol-spec` と自前定義の
  `RawHayate` ポート型だけ。**Hayate のランタイム/WASM adapter パッケージには依存しない。**
- **App が合成ルート。** 具体 adapter を Tsubame の renderer に注入するのは App。

### 3. 帰結としての再配置

- `init.ts` / `init-android.ts` / `probeWebGPU` / `resolveCanvasBackend` / `/android`
  サブパスは **Tsubame から退去**。Hayate 半分（WASM ロード・backend 選択・surface 取得）は
  Hayate web adapter / native へ。「CanvasRenderer 構築 + mount」半分は App entry へ。
- `@tsubame/renderer-canvas` は **`hayate-adapter-web` への依存を切る**。
- `CanvasRenderer` から `canvas` / `resize()` / `ResizeObserver` / `syncEditContext` /
  DPR / RAF 既定を撤去。受け取るのは `{ raw, clock }` のみ。`init-android.ts` は消滅。

### 4. resize と IME

- **resize は Tsubame の経路から外れる。** host→adapter→core が所有する（Hayate ADR-0080 を
  native まで延長）。Tsubame は `raw.on_resize` を直接叩かない。spec Contract には当面入れ
  ない（必要になった時に API 化）。`resize` を「Renderer Protocol surface」と呼ぶ既存記述
  （ADR-0053/0055・両 CONTEXT.md）は誤分類として訂正する。codegen 対象外である点は不変。
- **IME / EditContext は adapter が所有する**（ADR-0080 と同じ自己配線。web adapter が
  EditContext を、native が GameTextInput を）。`CanvasRenderer.frame()` から
  `syncEditContext` を撤去。

### 5. 命名

`Canvas Renderer` / `@tsubame/renderer-canvas` は維持する。"Canvas" は HTML `<canvas>` では
なく Hayate が描く即時描画サーフェス（"Hayate canvas" / h-canvas; Android `Canvas`・Skia・
Flutter `Canvas` と同義）を指す、とグロッサリで明示する。

### 6. ADR-0112 への影響

ADR-0112 の本筋（Android は埋め込み Hermes/JSI で native RawHayate を駆動、native が
vsync/GPU present を所有）は維持。**JS 側 glue（`init-android.ts` / `main.android.tsx`）の
所属だけを訂正** — Tsubame core ではなく App / native host bootstrap に属する。

## Considered Options

- **現状維持（`initCanvasRenderer` が一括で WASM ロード〜renderer 構築〜返却）**: 呼び出し
  側 1 行で済む利便はあるが、Tsubame⊥Hayate の境界を溶かし、`canvas: null` 無効化と
  `/android` サブパス分離という症状を生む。利便はフロントロードの便宜であって構造的価値で
  はないため却下。
- **`canvas: null` 無効化を正とする**: 「知識を型に残したまま実行時に殺す」設計。原則
  「Tsubame は host を知らない」を型レベルで破り続けるため却下。

## Consequences

### Positive

- どのファイルが誰のものかが初めて規則化され、以降の設計判断がこの上に乗る。
- browser / native の bootstrap が「host が raw(+clock) を渡し、App が renderer を構築して
  mount」で対称になる。`/android` サブパスも `canvas: null` 無効化も不要になる。
- `CanvasRenderer` が host 盲目の純粋な「flush→render→poll の tick オーケストレータ」に痩せ、
  テスト・型から `HTMLCanvasElement` が消える。

### Negative / フォローアップ

- **Hayate 側作業**（context 跨ぎ）: web adapter への EditContext 自己配線移管、web host
  bootstrap ヘルパ（WASM ロード〜`RawHayate` 返却）の新設、resize の adapter 抽象化。これらは
  Hayate/docs/adr 側で別途記録する。
- 公開 API 変更: `@tsubame/renderer-canvas` の export 面（`initCanvasRenderer` 等の退去）。
- App entry（`main.tsx` / `main.android.tsx`）が合成ルートとして Hayate ランタイムと Tsubame
  renderer を結ぶ薄い 2〜3 行になる。
- Rust サンドボックスに Android SDK/NDK/実機は無く、native 側は host 可読な契約テストで検証
  （ADR-0112 と同方針）。
