# Tsubame — 設計仕様書 v1.0

---

## 0. 哲学

> **"どのフレームワークも、どの GPU にも"**

Tsubame はフレームワークを選ばない。SolidJS でも Vue でも React でも、既存のランタイムとエコシステムをそのまま持ち込んでよい。

Tsubame はレンダリング先を選ばない。軽量な DOM CSR でも、WebGPU による GPU 描画でも、コンポーネントコードを変えずに切り替えられる。

Tsubame の責務は Renderer Protocol・DOM Renderer・Canvas Renderer の 3 つのみである。Signal・コンポーネントモデル・スケジューラは各フレームワークが持ち込む。

---

## 1. 問題定義

### 1.1 既存の JS フレームワーク × GPU 描画の課題

既存の JS フレームワーク（SolidJS / Vue / React）を Hayate（GPU Canvas）に対応させようとすると、各フレームワークのランタイムごとに Canvas 対応を個別実装する必要がある。これは実装コストが高く、エコシステムとの断絶を生む。

### 1.2 Signal 統一の罠（ADR-0038 → ADR-0040 の経緯）

「Signal ランタイムを統一すれば Canvas Renderer の実装が一回で済む」という仮説があった（ADR-0038）。しかし UI コンポーネント（マークアップを含む）は記法が異なるため adapter をまたいで共有できない。Signal 統一がもたらす主要メリットは存在せず、一方で Vue の Pinia / React の TanStack Query 等の 3rd party エコシステムが全滅するコストを伴う。そのため ADR-0040 で Signal 統一設計を廃棄した。

### 1.3 JS→WASM 境界コスト（ADR-0035 / ADR-0039 の経緯）

JS から Hayate WASM に mutation を毎回送ると JS→WASM 境界コストが O(N) になる。Canvas Renderer では JS 側でフレーム分をバッチ化し、`apply_mutations(ops: Float64Array, styles: Float32Array)` で 1回/frame に集約することで O(1)/frame に削減する。

---

## 2. Tsubame の定義

### 2.1 What Tsubame IS

```
Tsubame = Renderer Protocol (IRenderer)
        + DOM Renderer（Hayate 不使用、純 JS CSR）
        + Canvas Renderer（→ Hayate apply_mutations）
```

**Tsubame はレンダリングの "切替口" である。**

各フレームワークのランタイムはそのままに、レンダリング先だけを差し替える。React Native・NativeScript-Vue・SolidJS Universal と同じ実証済みパターンである。

### 2.2 What Tsubame IS NOT

- ❌ Signal ランタイム（各 Adapter フレームワークが持ち込む）
- ❌ コンポーネントモデル（各 Adapter フレームワークが持ち込む）
- ❌ スケジューラ（各 Adapter フレームワークが持ち込む）
- ❌ フレームワーク（Adapter の上に乗るフレームワークが UI を定義する）
- ❌ Hayabusa の JS 版（Hayabusa は WASM 専用・Rust/Python 向け）

---

## 3. アーキテクチャ

```
tsubame-solid         tsubame-vue              tsubame-react
（SolidJS runtime    （@vue/reactivity +       （React Fiber +
  solid-js/universal） createRenderer()）        react-reconciler）
        ↓                    ↓                        ↓
              Renderer Protocol (IRenderer)
                    ↓                ↓
             DOM Renderer      Canvas Renderer
            （直接 DOM 操作）  （→ apply_mutations → Hayate → WebGPU）
```

### 3.1 Renderer Protocol

TypeScript の `interface IRenderer` として定義する。Adapter はこのインターフェースを通じてのみレンダリングを行い、DOM か Canvas かを意識しない。

主要な操作:
- `createElement(kind: ElementKind): ElementId`
- `appendChild(parent: ElementId, child: ElementId): void`
- `insertBefore(parent: ElementId, child: ElementId, before: ElementId): void`
- `removeChild(parent: ElementId, child: ElementId): void`
- `setStyle(id: ElementId, style: StylePatch): void`
- `setText(id: ElementId, text: string): void`
- `addEventListener(id: ElementId, event: EventKind, handler: EventHandler): void`

### 3.2 DOM Renderer

- 純 JS、Hayate 不使用
- `createElement` → `document.createElement`（Hayate element vocabulary を HTML タグにマッピング）
- `setStyle` → `element.style` への直接書き込み
- JS→WASM 境界なし
- SSG・SSR・ハイドレーションは行わない

### 3.3 Canvas Renderer

- Hayate WASM を使用
- フレームごとに mutation を `ops: Float64Array` / `styles: Float32Array` に積算
- `requestAnimationFrame` コールバック内で `apply_mutations(ops, styles)` を 1回/frame 呼び出す
- `element_create` は戻り値（ElementId）が必要なため個別呼び出し
- 文字列 op（`element_set_text` 等）は頻度が低いため個別呼び出し

#### apply_mutations ops ストリーム

固定長レコードの繰り返し。各レコードは `op_kind` から始まり op 種別ごとの固定 slot 数を消費する。

| op_kind | slots | layout |
|---------|-------|--------|
| OP_APPEND_CHILD      | 3 | `op, parent_id, child_id` |
| OP_INSERT_BEFORE     | 4 | `op, parent_id, child_id, before_id` |
| OP_REMOVE            | 2 | `op, id` |
| OP_SET_ROOT          | 2 | `op, id` |
| OP_SET_STYLE         | 4 | `op, id, style_offset, style_len` |
| OP_SET_TRANSFORM     | 9 | `op, id, has_matrix, m0, m1, m2, m3, m4, m5` |
| OP_SET_SCROLL_OFFSET | 4 | `op, id, x, y` |
| OP_FOCUS             | 2 | `op, id` |
| OP_BLUR              | 2 | `op, id` |

`styles` バッファは Hayate の `style_packet.rs` の TAG エンコーディング（flat f32 配列）をそのまま使用する。

---

## 4. Tsubame Adapter 一覧

### 4.1 tsubame-solid

- SolidJS の `solid-js/universal` カスタムレンダラー API を使用
- SolidJS の fine-grained signals / `onMount` / `onCleanup` はそのまま動く
- Solid Router・SolidQuery 等 SolidJS エコシステムがそのまま動く
- `.tsx` 形式。コンポーネント関数は一度だけ実行（Virtual DOM なし）
- 実装順: **優先（旧 Tsubame JSX 層の引き継ぎ）**

### 4.2 tsubame-vue

- `@vue/runtime-core` の `createRenderer()` API を使用
- `@vue/reactivity`（`ref`/`computed`/`watchEffect`）はそのまま動く
- Pinia・VueUse・VueRouter 等 Vue エコシステムがそのまま動く
- `.vue` SFC 形式。`<template>` は `@vue/compiler-dom` のコードジェネレータを差し替えて Renderer Protocol 呼び出しに変換
- 実装順: **2番目（開発者数・実装コストのバランス最良）**

### 4.3 tsubame-react

- `react-reconciler` を使用
- React の Fiber ランタイム（hooks・Suspense・Context 等）はそのまま動く
- TanStack Query・Zustand・Jotai 等 React エコシステムがそのまま動く
- JSX/TSX 形式
- 実装順: **3番目（開発者数最多だが競合多）**

### 4.4 tsubame-svelte（スコープ外）

Svelte の価値の大半はコンパイラ最適化と `.svelte` 構文にある。コンパイラ改造の工数に対してメリットが薄いためスコープ外とし、Svelte ユーザーには tsubame-vue を推奨する。

---

## 5. Element 語彙

Hayate の element vocabulary（React Native 語彙）を統一して使用する。

| Tsubame / Hayate | DOM Renderer マッピング |
|------------------|------------------------|
| `view`           | `<div>` |
| `text`           | `<span>` |
| `image`          | `<img>` |
| `button`         | `<button>` |
| `text-input`     | `<input>` |
| `scroll-view`    | `<div style="overflow: auto">` |

HTML タグ名（div / span / p / h1 等）は Tsubame API には露出しない。

---

## 6. Hayate との関係

Tsubame と Hayate は完全に独立したリポジトリである。

| 項目 | 詳細 |
|------|------|
| 結合点 | `apply_mutations(ops: Float64Array, styles: Float32Array)` の仕様のみ |
| 依存方向 | Tsubame → Hayate（一方向）。Hayate は Tsubame を知らない |
| DOM Renderer | Hayate を一切使用しない |
| Canvas Renderer | Hayate WASM を `import` して使用 |

---

## 7. リポジトリ構成

```
packages/
  renderer-protocol/   ← IRenderer インターフェース + ElementKind + StylePatch 型定義
  renderer-dom/        ← DOM Renderer 実装
  renderer-canvas/     ← Canvas Renderer 実装（apply_mutations バッチャー含む）
  solid/               ← tsubame-solid
  vue/                 ← tsubame-vue
  react/               ← tsubame-react
```

純 JS モノレポ。ビルドツールは未確定（pnpm workspaces + Vite が候補）。

---

## 8. 実装ロードマップ

| Phase | 内容 |
|-------|------|
| T1 | Renderer Protocol 型定義（`packages/renderer-protocol`） |
| T2 | DOM Renderer 実装（`packages/renderer-dom`） |
| T3 | tsubame-solid 実装（`packages/solid`） |
| T4 | Canvas Renderer 実装（`packages/renderer-canvas`、Hayate WASM 連携） |
| T5 | WASM バインディング動作確認（`apply_mutations` 引数型） |
| T6 | Hello World デモ（DOM / Canvas 両 Renderer） |
| T7 | tsubame-vue 実装（`packages/vue`） |
| T8 | tsubame-react 実装（`packages/react`） |
