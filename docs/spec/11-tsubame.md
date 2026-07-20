# §11 Tsubame

JS/TS 向けのレンダラーターゲット基盤。Renderer Protocol・DOM Renderer・Canvas Renderer の3責務に限る。
Hayate との結合点（apply_mutations / poll_events）の wire は §10。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### TSUB-01 — レンダラーターゲット基盤（signal ランタイムではない）
**規範文:** Tsubame は Hayate と独立した pure JS モノレポであり、`Renderer Protocol`（`IRenderer`）・DOM Renderer・Canvas Renderer の3層のみを責務とする。signal・component model・scheduler は各フレームワークが持ち込む。
**出典:** Tsubame ADR-0002, Hayate ADR-0040（ADR-0038 を supersede）
**状況:** ✅ — `Tsubame/packages/`：renderer-protocol / renderer-dom / renderer-canvas / solid / react。`tsubame-spec.md` が責務を明示。host bootstrap（WASM ロード・backend 選択・surface 取得）は ADR-0004 で Tsubame から退去し Hayate 側（`@torimi/hayate-host`）と App entry が所有する。
**備考:** [履歴 C-11.2] Hayate ADR-0038「Tsubame を signal 統一ランタイムに」は ADR-0040 が supersede（記法差で adapter 間共有不可、Vue/React ecosystem が全滅するため）。

### TSUB-02 — Renderer Protocol（IRenderer）／property は閉じた語彙
**規範文:** `IRenderer` は element 作成・ツリー操作（appendChild/insertBefore/removeChild/setRoot）・スタイル（setStyle/setStyleVariant/setPseudoStyle/setText）・property・イベント購読を抽象化する。adapter はこの interface を通じてのみ描画し、DOM か Canvas かを意識しない。`resize` は `IRenderer` の表面に含めない — surface サイズ追従は host 側 adapter が所有する責務であり Renderer Protocol の領分ではない（ADR-0004 が ADR-0053/0055 の「resize＝Renderer Protocol surface」記述を誤分類として訂正、TSUB-08）。**element property は閉じた typed 語彙**とし、既知の意味プロパティ（現状 `value`/`placeholder`/`disabled`/`src`）を両 renderer が実装する。`aria-label`/`role` は first-class API 経由で接続する設計だが**未実装**（下記 状況）。**未知 property 名はエラー**（任意 HTML 属性のフォールバックは禁止＝ELEM-01 のタグ禁止と同格）。
**出典:** Tsubame ADR-0002、ADR-0071（property 閉じた語彙）、ADR-0004（resize を Renderer Protocol から除外）
**状況:** 🟡 — `property.ts` の `ELEMENT_PROPERTY_NAMES`（`value`/`placeholder`/`disabled`/`src`）と `assertKnownElementProperty`（未知 throw）を DOM/Canvas renderer と `tsubame-solid` が共有（`dom-renderer.property.test.ts` / `canvas-renderer.test.ts`）。DOM の任意 `setAttribute` フォールバック撤去済み。Canvas の silent drop 撤去済み。**残:** `aria-label`/`role` の first-class `IRenderer` API と Canvas wire 経路（Hayate WASM の `element_set_aria_label`/`element_set_role` は存在するが Tsubame 未接続。`setProperty('aria-label')` は意図的に throw）。
**備考:** ADR-0071。setPseudoStyle のキー単位 API はアダプタ側に分割ロジックを漏らす（別改善）。

### TSUB-03 — DOM Renderer（CSR、Hayate 不使用）
**規範文:** DOM Renderer は HTML 直接操作で CSR を行い、Hayate（WASM）を一切使わない。element-kind を HTML tag に 1:1 マップ（view→div / text→span / button→button / text-input→input / image→img / scroll-view→div）し、各要素に `data-tsubame-id` を付与して `ElementId` を保持する。
**出典:** Tsubame ADR-0002
**状況:** ✅ — `renderer-dom/src/dom-elements.ts`（kind→tag）、`dom-renderer.ts` の `elementIdFromDom`（`data-tsubame-id`）。
**備考:** Hayate HTML Mode（§8）とは別概念（Hayate 不関与）。

### TSUB-04 — Canvas Renderer（apply_mutations を1回/frame）
**規範文:** HayateRenderer は JS 内でフレーム分の mutations を自身の semantic queue に積み、frame-clock の tick ごとに `apply_mutations(ops, styles, texts)` を1回呼ぶ。これで JS→WASM 境界を O(1)/frame にする。Canvas Renderer は **host 盲目**のコアであり、構築入力は `{ raw, requestFrame, cancelFrame }` のみ（surface/canvas・resize・IME・pointer・DPR・`ResizeObserver`・RAF 既定は持たない）。frame-clock は host が確立し注入する。構築は副作用なしで、ループは明示 `start()`/`stop()` でのみ駆動する（native は vsync 準備後に開始）。
**出典:** Tsubame ADR-0002, ADR-0003（→§10）, ADR-0004（host-blind コア・clock 注入）
**状況:** ✅ — `renderer-hayate/src/hayate-renderer.ts`（`HayateRenderer({ raw, requestFrame, cancelFrame })`・`start()`/`stop()`・`frame()` で flush→prepare/commit） が自身の semantic queue を所有する。`HTMLCanvasElement` 型・`canvas` 参照・`ResizeObserver` はコアに不在（#476/#477、ADR-0004）。
**備考:** wire 詳細・検証は §10（PROTO-04〜11）。host bootstrap（surface 取得・WASM ロード・backend 選択・clock 源）は App entry ＋ Hayate `@torimi/hayate-host` が所有（ADR-0004・WEBA-01）。

### TSUB-05 — adapter は既存ランタイムを持ち込む
**規範文:** 各 Tsubame Adapter は自身のフレームワークの既存ランタイム（SolidJS signals / Vue reactivity / React Fiber）をそのまま維持し、レンダリング先のみ Renderer Protocol に向け替える。signal 統一はしない。
**出典:** Tsubame ADR-0004, Hayate ADR-0040, Tsubame ADR-0010（tsubame-react reconciler）
**状況:** 🟡 — `tsubame-solid`（`solid-js/universal` カスタムレンダラー、`solid-js` 依存維持）と `tsubame-react`（`react-reconciler` HostConfig、`packages/react/src/host-config.ts`・write-only・構造ゼロ instance・container 生成時に renderer 束縛、ADR-0010）が実装済み。`tsubame-react` の登録/swap/removal/rejection・semantic props・style/pseudo/styleVariants・event は `IRenderer` 境界でテスト固定（`host-config.test.tsx` / `instance.test.tsx`）。`tsubame-vue` は ⬜未実装（packages に不在）。
**備考:** tsubame-svelte はスコープ外（Svelte ユーザーには tsubame-vue 推奨）。`tsubame-solid` は `TsubameNode` を**構造専用 shadow tree**（reconcile index）として保持する — `solid-js/universal` が VDOM を持たず reconcile 時にホスト構造を同期で読むため、batch 境界越しに正本を置く Canvas 経路では不可避（ADR-0062 が ADR-0057 を supersede、§2 ELEM-03）。VDOM reconciler の `tsubame-react`（実装済み）/ `tsubame-vue` は shadow 不要（ADR-0010 が tsubame-react の write-only・構造ゼロ instance を確定）。

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

### TSUB-08 — ビューポートのサイズ追従は host 側 adapter の責務（Tsubame 責務外）
**規範文:** ビューポート/surface のサイズ追従は **host 側 adapter が所有**し、Tsubame の Renderer Protocol の経路から外す。Hayate が surface を描く可搬経路（Canvas）では、resize・DPR・`ResizeObserver`・EditContext は host adapter（web は EditContext と `ResizeObserver` を自己配線、native は GameTextInput / `CADisplayLink` 等）が持ち、host→adapter→core が resize を所有する（ADR-0080 を native まで延長）。Tsubame Canvas Renderer は `raw.on_resize` を直接叩かず、`{ raw, clock }` のみを受ける host 盲目のコアに留まる（TSUB-04）。DOM Renderer は host == 描画先（DOM）でブラウザの reflow に委ね、対称のサイズ追従コードを持たない。レイアウト座標は CSS px・描画は物理 px。アプリコードはどちらのレンダラーでもサイズ管理を書かない（レンダラー間パリティ）。
**出典:** ADR-0004（host bootstrap 退去・resize を adapter 抽象へ）、ADR-0080（adapter 自己配線）、ADR-0007（旧 CanvasRenderer 所有を supersede）
**状況:** 🟡 — Tsubame 側の退去は完了（`canvas-renderer.ts` から `canvas` / `resize()` / `ResizeObserver` / `syncEditContext` / DPR / RAF 既定を撤去、`{ raw, requestFrame, cancelFrame }` のみ受領・#476/#477）。**残（Hayate 側・context 跨ぎ）:** web adapter への EditContext/`ResizeObserver` 自己配線移管と resize の adapter 抽象化は ADR-0004 が「フォローアップ」として記録（Hayate/docs/adr 側で別途）。
**備考:** [更新] ADR-0007「ビューポートのサイズ追従は CanvasRenderer の責務（`ResizeObserver`＋`devicePixelRatio`）」は ADR-0004 が supersede — 「知識を型に残し実行時に殺す（native の `canvas: null` 無効化）」設計を排し、Canvas Renderer を host 盲目に痩せさせた。resize を「Renderer Protocol surface」と呼ぶ ADR-0053/0055・両 CONTEXT.md の記述は誤分類として訂正対象（codegen 対象外である点は不変、§10 PROTO-18）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 5 | TSUB-01, 03, 04, 06, 07 |
| 🟡部分 | 3 | TSUB-02（property 閉じ語彙・aria first-class 未接続、ADR-0071）、TSUB-05（solid・react 実装済み／vue 未実装、ADR-0010）、TSUB-08（Tsubame 退去済み・Hayate 側 resize 抽象は follow-up、ADR-0004） |
| ⬜未実装 | 0 | — |
