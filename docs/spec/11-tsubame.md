# §11 Tsubame

JS/TS 向けのレンダラーターゲット基盤。Renderer Protocol・DOM Renderer・Canvas Renderer の3責務に限る。
Hayate との結合点（apply_mutations / poll_events）の wire は §10。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### TSUB-01 — レンダラーターゲット基盤（signal ランタイムではない）
**規範文:** Tsubame は Hayate と独立した pure JS モノレポであり、`Renderer Protocol`（`IRenderer`）・DOM Renderer・Canvas Renderer の3層のみを責務とする。signal・component model・scheduler は各フレームワークが持ち込む。
**出典:** Tsubame ADR-0002, Hayate ADR-0040（ADR-0038 を supersede）
**状況:** ✅ — `Tsubame/packages/`：renderer-protocol / renderer-dom / renderer-canvas / solid。`tsubame-spec.md` が責務を明示。
**備考:** [履歴 C-11.2] Hayate ADR-0038「Tsubame を signal 統一ランタイムに」は ADR-0040 が supersede（記法差で adapter 間共有不可、Vue/React ecosystem が全滅するため）。

### TSUB-02 — Renderer Protocol（IRenderer）
**規範文:** `IRenderer` は element 作成・ツリー操作（appendChild/insertBefore/removeChild/setRoot）・スタイル（setStyle/setPseudoStyle/setText）・プロパティ・イベント購読・resize を抽象化する。adapter はこの interface を通じてのみ描画し、DOM か Canvas かを意識しない。
**出典:** Tsubame ADR-0002
**状況:** 🟡 — interface は `renderer-protocol/src/renderer.ts` に定義済（`setPseudoStyle` で `:hover`/`:active`/`:focus` 分離）。**ただし `setProperty` の実装が adapter 間で非対称** — DOM Renderer は実装済（`dom-renderer.ts:126`、`value`/`placeholder`/`disabled`/`src` を処理）だが、Canvas Renderer は no-op（`canvas-renderer.ts:99`）で同プロパティを silent drop する。規範文「adapter は DOM か Canvas かを意識しない」が property 経路で未達。
**備考:** setPseudoStyle のキー単位 API はアダプタ側に分割ロジックを漏らす（アーキテクチャレビュー候補5、§改善）。残タスク: Canvas 側で `setProperty` を semantic mutation として encode するか、未対応を型レベルで明示する。

### TSUB-03 — DOM Renderer（CSR、Hayate 不使用）
**規範文:** DOM Renderer は HTML 直接操作で CSR を行い、Hayate（WASM）を一切使わない。element-kind を HTML tag に 1:1 マップ（view→div / text→span / button→button / text-input→input / image→img / scroll-view→div）し、各要素に `data-tsubame-id` を付与して `ElementId` を保持する。
**出典:** Tsubame ADR-0002
**状況:** ✅ — `renderer-dom/src/dom-elements.ts`（kind→tag）、`dom-renderer.ts` の `elementIdFromDom`（`data-tsubame-id`）。
**備考:** Hayate HTML Mode（§8）とは別概念（Hayate 不関与）。

### TSUB-04 — Canvas Renderer（apply_mutations を1回/frame）
**規範文:** Canvas Renderer は JS 内でフレーム分の mutations を `HayateMutationPacket` に積み、`requestAnimationFrame` ごとに `apply_mutations(ops, styles, texts)` を1回呼ぶ。これで JS→WASM 境界を O(1)/frame にする。
**出典:** Tsubame ADR-0002, ADR-0003（→§10）
**状況:** ✅ — `renderer-canvas/src/canvas-renderer.ts` + `hayate-mutation-packet.ts`（enqueue→flush）。
**備考:** wire 詳細・検証は §10（PROTO-04〜11）。

### TSUB-05 — adapter は既存ランタイムを持ち込む
**規範文:** 各 Tsubame Adapter は自身のフレームワークの既存ランタイム（SolidJS signals / Vue reactivity / React Fiber）をそのまま維持し、レンダリング先のみ Renderer Protocol に向け替える。signal 統一はしない。
**出典:** Tsubame ADR-0004, Hayate ADR-0040
**状況:** 🟡 — `tsubame-solid` は実装済み（`solid-js/universal` カスタムレンダラー、`solid-js` 依存維持）。`tsubame-vue` / `tsubame-react` は ⬜未実装（packages に不在）。
**備考:** tsubame-svelte はスコープ外（Svelte ユーザーには tsubame-vue 推奨）。

### TSUB-06 — DOM Renderer の Z-Order は RN Web 方式エミュレート
**規範文:** DOM Renderer は RN Web 現行方式で Z-Order をエミュレートする。全 element kind に `position: relative` + `zIndex: 0` をベース付与し、開発者指定 zIndex で上書き、兄弟内のみ有効とする。ブラウザ CSS との完全一致は目標にしない。
**出典:** Tsubame ADR-0006（origin: Hayate ADR-0021）
**状況:** ✅ — `renderer-dom/src/dom-elements.ts` の BASE_STYLE（`position:relative` / `zIndex:0`）、`z-order-divergence` で opacity/transform 由来の暗黙 stacking を警告。
**備考:** Canvas Renderer は Hayate `scene_build` が同一セマンティクスを保証（§4 REND-03）。

### TSUB-07 — ElementId は JS 側がモノトニック採番
**規範文:** `ElementId` は JS 側がモノトニックカウンター（初期値1）で採番し、`OP_CREATE` で WASM に通知する。WASM は採番済み id をそのまま受け取る。
**出典:** Tsubame ADR-0005
**状況:** ✅ — DOM/Canvas 両 renderer で `private nextId = 1` → `createElement` で `nextId++`。
**備考:** [履歴 C-11.3] 旧案は WASM が `element_create→ElementId` を同期返却。JS 採番で createElement を batch 内に乗せ境界を1回/frame に削減（§10 PROTO-06 と整合）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 5 | TSUB-01, 03, 04, 06, 07 |
| 🟡部分 | 2 | TSUB-02（Canvas `setProperty` が no-op で `src`/`value`/`placeholder`/`disabled` を silent drop）, TSUB-05（solid のみ実装、vue/react 未実装） |
