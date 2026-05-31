# Tsubame（燕）

**Tsubame（燕）** は、JS/TS 向けの**レンダラーターゲット基盤**である。フレームワークでも signal ランタイムでもなく、Renderer Protocol（`IRenderer`）・DOM Renderer・Canvas Renderer の 3 つを提供する層である。各 Tsubame Adapter は自身のフレームワーク固有のランタイム（SolidJS の signals / Vue の `@vue/reactivity` / React の Fiber）をそのまま持ち込み、レンダリング先を Tsubame の Renderer Protocol に向け替える。Tsubame は signal・コンポーネントモデル・スケジューラを持たない。Hayabusa・Hayate コアのいずれも Tsubame の存在を知らない。Hayate とは完全に独立した別リポジトリ（pure JS モノレポ）。
_Avoid_: signal ランタイム、フレームワーク、React hooks ベース、Virtual DOM を持たない（adapter が持ち込む）

## Language

**Renderer Protocol**:
Tsubame と Tsubame Adapter の間の境界インターフェース。element の作成・ツリー操作・スタイル設定・イベント購読を抽象化した仕様。TypeScript では `interface IRenderer` として定義される。DOM Renderer と Canvas Renderer の二つの実装を持つ。Tsubame Adapter はこのプロトコルを通じてのみレンダリングを行い、DOM か Canvas かを意識しない。
_Avoid_: Host Interface, Host Config, Element Driver

**ElementId**:
Renderer Protocol 全体で element を識別する opaque 型。`type ElementId = number & { __brand: 'ElementId' }`。JS 実行時は number のままでゼロオーバーヘッド。Canvas Renderer の `ops: Float64Array` への格納時は `id as number` で unwrap する。
_Avoid_: string id、グローバル連番以外の実装（内部実装の話であり Protocol の定義には含めない）

**StylePatch**:
`IRenderer.setStyle` の第二引数。`{ [K in keyof HayateStyle]?: HayateStyle[K] | null }` 型。指定されたプロパティのみ上書き、未指定は変更なし、`null` はリセット（デフォルト値に戻す）を意味する。
_Avoid_: フル置換（毎フレーム全プロパティを送る設計）

**DOM Renderer**:
Renderer Protocol の実装の一つ。CSR（Client-Side Rendering）のみ。Signal が DOM を直接操作する。Hayate（WASM）を一切使用しない。JS→WASM 境界が存在しない。SSG・SSR・ハイドレーションは行わない。Hayate の HTML Mode とは別概念であり、Hayate が関与しない点が根本的に異なる。
_Avoid_: Tsubame DOM Mode, SSG, SSR, ハイドレーション, Hayate HTML Mode（Hayate 不使用のため）

**Canvas Renderer**:
Renderer Protocol の実装の一つ。JS 内でフレーム分の mutations を積み、`apply_mutations(ops: Float64Array, styles: Float32Array)` で Hayate（WASM）に 1回/frame で渡す。JS→WASM 境界のコストを O(N) から O(1)/frame に削減する。Hayate の Canvas Mode（WebGPU GPU 描画）と組み合わせて動作する。Renderer Protocol を実装しているため、Tsubame Adapter はどちらの Renderer を使うかを意識しない。
_Avoid_: Tsubame Canvas Mode, 個別 element_set_* 呼び出し（Canvas Renderer では JS 側でバッチ化する）

**Tsubame Adapter**:
各フレームワークの既存ランタイムを Hayate（GPU Canvas）と DOM の両方にターゲットさせるブリッジ層。`tsubame-solid` / `tsubame-vue` / `tsubame-react` の 3 つを指す（tsubame-svelte はスコープ外。Svelte ユーザーには tsubame-vue を推奨）。各 adapter は自身のフレームワークのエコシステム（Pinia・TanStack Query 等の 3rd party ライブラリを含む）をそのまま維持し、レンダリング先を Tsubame の Renderer Protocol に向け替えるだけである。コンポーネントの UI ロジックは adapter をまたいで共有しない（記法が異なるため定義上不可能）。Tsubame リポジトリ内のモノレポ（`packages/renderer-protocol` / `packages/renderer-dom` / `packages/renderer-canvas` / `packages/solid` / `packages/vue` / `packages/react`）として管理される。Hayate リポジトリとは完全に独立した別リポジトリであり、結合点は `apply_mutations` の仕様のみ。
_Avoid_: Solid-native, Vue-native（既存プロジェクト名との衝突を避けるため）, tsubame-svelte, signal共有（各adapterが独自ランタイムを持つため）

**tsubame-solid**:
Tsubame Adapter の一つ。SolidJS の `solid-js/universal` カスタムレンダラー API を使い、SolidJS のランタイム（fine-grained signals / `onMount` / `onCleanup` 等）をそのまま維持しつつレンダリング先を Tsubame の Renderer Protocol に向け替える。SolidJS のエコシステム（Solid Router・SolidQuery 等）がそのまま動く。`.tsx` 形式・コンポーネント関数は一度だけ実行・Virtual DOM なし。
_Avoid_: Tsubame signal への依存（SolidJS 自身の signal を使う）

**tsubame-vue**:
Tsubame Adapter の一つ。`@vue/runtime-core` の `createRenderer()` API を使い、Vue のランタイム（`@vue/reactivity` の `ref`/`computed`/`watchEffect`・VDOM・コンポーネントライフサイクル）をそのまま維持しつつレンダリング先を Tsubame の Renderer Protocol に向け替える。Pinia・VueUse・VueRouter 等の Vue エコシステムがそのまま動く。`.vue` SFC 形式を採用し、`<template>` 内では Element 語彙（`<view>` / `<text>` 等）をそのまま使う。`@vue/compiler-dom` のコードジェネレータ差し替えは MVP スコープ外。
_Avoid_: @vue/reactivity を Tsubame signal に置き換える設計（Vue エコシステムが全滅するため）、MVP でのコンパイラ改造

**tsubame-react**:
Tsubame Adapter の一つ。`react-reconciler` を使い、React の Fiber ランタイム（hooks・Suspense・Context 等）をそのまま維持しつつレンダリング先を Tsubame の Renderer Protocol に向け替える。TanStack Query・Zustand・Jotai 等の React エコシステムがそのまま動く。JSX/TSX 形式。既存の React コードを最小限の変更で Hayate（GPU Canvas）と DOM に対応させられる。
_Avoid_: Tsubame signal への依存（React 自身の Fiber ランタイムを使う）、hooks 互換シム（React 本体を使うため不要）

**apply_mutations**:
Canvas Renderer が毎フレーム Hayate WASM に渡す JS→WASM 境界の唯一の hot path。シグネチャは `apply_mutations(ops: Float64Array, styles: Float32Array)`。JS 側でフレーム分の mutations を積算してから 1回/frame で呼び出すことで、境界コストを O(N) から O(1)/frame に削減する。`ops` は固定長レコードの繰り返しストリーム（OP_APPEND_CHILD / OP_INSERT_BEFORE / OP_REMOVE / OP_SET_ROOT / OP_SET_STYLE / OP_SET_TRANSFORM / OP_SET_SCROLL_OFFSET / OP_FOCUS / OP_BLUR）。`styles` は flat f32 配列（`style_packet.rs` の TAG エンコーディング）。文字列 op（`element_set_text` 等）と `element_create`（戻り値が必要）はバッチ外で個別呼び出しを維持する。
_Avoid_: 毎 mutation ごとの wasm 呼び出し、JSON エンコーディング

**Element**:
Tsubame が扱う UI の構成単位。Hayate の Element Layer に対応し、React Native 語彙（`view` / `text` / `image` / `button` / `text-input` / `scroll-view`）を採用する。HTML タグ名（div / span 等）は使用しない。Tsubame Adapter はこの element 型を Renderer Protocol 経由で操作する。
_Avoid_: div, span, HTML タグ

**Hayate CSS**:
Tsubame が Canvas Renderer 経由で Hayate に渡すスタイル仕様。レイアウトプロパティ（display / gap / align-items / grid-template-columns 等）は Taffy の CSS Flexbox / Grid / Block 実装を仕様書とする。ビジュアルプロパティ（color / background-color / border-radius / opacity 等）は CSS プロパティ名を踏襲しつつ Hayate が対応サブセットを定義する。DOM Renderer では対応する CSS プロパティに直接マッピングする。
_Avoid_: CSS、CSS 風スタイル

**HayateStyle（MVP サブセット）**:
MVP で対応する `HayateStyle` のプロパティ一覧。レイアウト: `width` / `height` / `display` / `flexDirection` / `alignItems` / `justifyContent` / `gap`。ビジュアル: `color` / `backgroundColor` / `borderRadius` / `opacity`。テキスト: `fontSize` / `fontWeight`。Grid・overflow・border・shadow 等は MVP 後。
_Avoid_: MVP での全プロパティ実装

**Interaction Event**:
ポインタやキーボード操作に起因する要素単位のイベント。Hayate の `poll_events()` で上位層に通知される。Canvas Renderer 使用時は Hayate からイベントを受け取り Adapter のイベントシステムに橋渡しする。インタラクション状態に応じたスタイル切り替えは各 Adapter フレームワークの reactivity の責務。MVP の `EventKind`: `click` / `hover-enter` / `hover-leave` / `focus` / `blur`。MVP 後に追加予定: `keydown` / `keyup` / `scroll` / `active-start` / `active-end`。
_Avoid_: :hover スタイル、状態付きスタイル

## Example Dialogue

> 「Tsubame は SolidJS の代替か？」
> → 「違う。SolidJS のランタイムをそのまま使い、レンダリング先を DOM から Hayate（GPU）または DOM に切り替える基盤が Tsubame。SolidJS のエコシステムはそのまま動く」

> 「Vue プロジェクトを Hayate（GPU Canvas）に対応させられるか？」
> → 「できる。tsubame-vue を使えば `@vue/reactivity` / Pinia / VueRouter がそのまま動く。`createRenderer()` でレンダリング先を Canvas Renderer に向け替えるだけ」

> 「DOM Renderer と Canvas Renderer でコンポーネントコードを変える必要があるか？」
> → 「ない。Renderer Protocol が差異を吸収する。Adapter コードはどちらの Renderer を使うかを意識しない」

> 「tsubame-svelte は作るか？」
> → 「スコープ外。Svelte の価値の大半はコンパイラ最適化と `.svelte` 構文にあり、コンパイラ改造の工数に見合わない。Svelte ユーザーには tsubame-vue を推奨する」

> 「Hayate と Tsubame の結合点は何か？」
> → 「`apply_mutations(ops: Float64Array, styles: Float32Array)` の仕様のみ。Hayate は Tsubame の存在を知らない」
