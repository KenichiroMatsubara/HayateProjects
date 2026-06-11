# Hayate / Hayabusa

**Hayate（疾風）** は、アプリケーション UI のための命令型・保持型・GPU ネイティブな UI 基盤である。
**Hayabusa（隼）** は、Hayate の上で動く Signal ベース SFC フレームワークである。

Hayate は UI フレームワーク・状態管理・Reconciler・Component tree のいずれでもない。DOM 互換は設計目標に含まない。

> 語彙の正本。各語が**何であるか**を定義する。実装の仕組み・根拠・決定は `docs/spec/` と各 ADR に置き、ここには書かない。

## Language

**Hayate（疾風）**:
命令型・保持型・GPU ネイティブな UI 基盤。上位層が組み立てた element を受け取って描画する。外部公開サーフェスは Element Layer ベースの proto 契約の一つだけ。
_Avoid_: フレームワーク、ライブラリ、レンダラー単体

**Hayabusa（隼）**:
Hayate の Element Layer 上に構築された Signal ベースの SFC（Single-File Component）フレームワーク。`.hybs` 形式で、テンプレートとスタイルは言語非依存の DSL、スクリプトはプロジェクト単位で選んだ単一言語で書く。
_Avoid_: Hayate の別名、エンジン、Rust 専用フレームワーク

**Element Layer（要素層）**:
Hayate の外部公開サーフェス。element tree の作成・Hayate CSS の設定・ツリー組み立てを受け付ける上位の概念層。Hayabusa および他言語 SDK はこの層を使う。
_Avoid_: 上位 API、UI 層、Scene Layer

**Element（要素）**:
Element Layer が扱う UI の構成単位。`view` / `text` / `image` / `button` / `text-input` / `scroll-view` を基本型とする React Native 由来の語彙。HTML タグ名（div / span / input 等）は使わない。
_Avoid_: div, span, section, p, h1〜h6（HTML 語彙全般）

**Inline Formatting Context（IFC）**:
`text` element が確立する inline 整形単位。IFC root（親が `text` でない `text`）が subtree を 1 つの整形範囲としてまとめ、inline text element（親が `text` の `text`）はその styled range になる。シンタックスハイライト等の inline styled text を可能にする。
_Avoid_: leaf-string + 親 collapse モデル、inline text element を独立 box とする設計

**block box / inline text element**:
element の 2 クラス。block box（`view` / `button` / `image` / `scroll-view` / `text-input` および IFC root の `text`）はレイアウトボックスを 1 つ持つ。inline text element（IFC 内の子 `text`）はボックスを持たず親 IFC の range となる。
_Avoid_: 全 element = 1 レイアウトボックスという旧前提

**Taffy Projection**:
document ツリーの block-box 部分集合から lazy に派生するレイアウトツリー。document ツリーが構造・データの唯一の owner で、レイアウトツリーは peer ではなく派生 projection。
_Avoid_: レイアウトツリーを owner とする設計、layout ツリーと document ツリーの 1:1 視

**テキスト継承（2チャネル）**:
テキストスタイルの継承モデル。(1) 通常 text スタイル（`color` / `font-size` / `font-family` / `font-weight` / `font-style` / `text-decoration`）は text→text（IFC 内）のみ継承し block を貫通しない。(2) ambient Default Text Style（`default-*`）は任意 element から block を貫通して既定値を供給する。
_Avoid_: CSS 式の全要素 font 継承、ambient 既定なし設計

**Default Text Style（ambient）**:
`default-font-family` / `default-font-size` / `default-font-weight` / `default-color`。block を貫通して降りる既定値専用チャネルで、text 要素が明示値も text 継承値も持たないときの fallback。nested 上書き可。
_Avoid_: 通常 `font-family` 等を block 貫通させる設計、global（nested 上書き可能な ambient であり真の global ではない）

**Hayate CSS**:
要素ごとのインラインスタイル宣言。レイアウトプロパティは Taffy の CSS サブセット、ビジュアルプロパティは CSS 名の対応サブセット。要素ローカルの擬似状態（`:hover` / `:active` / `:focus`）を同一宣言内に nest できる。セレクタ・カスケード・スタイルシートは持たない。
_Avoid_: CSS（フル互換の含意）、CSS 風スタイル、Element Style

**Raw Layer（生座標層）**:
Hayate の内部 lowering target。絶対座標・確定スタイル済みの描画コマンド（`SceneGraph` / `Node`）。外部公開しない内部実装。
_Avoid_: 外部公開 API・公開サーフェス、Draw Layer

**WIT（WebAssembly Interface Types）**【歴史】:
Hayate の公開境界として**かつて**使っていた設計語彙。現行の Hayate–Tsubame プロトコル正本ではない（現行は proto 契約）。文書で言及する際は過去設計であることを明示する。
_Avoid_: 現行の公開 API・現行プロトコル正本

**Platform Adapter**:
Hayate Core とプラットフォームを仲介する層。IME 入力・クリップボード・raw 入力イベント変換を担い、プラットフォームごとに異なる実装を持つ。Core は Platform Adapter を知らない。
_Avoid_: Runtime, Host, Surface Adapter

**Canvas Mode**:
`hayate-adapter-web` の動作モードの一つ。Vello + wgpu（WebGPU）で全 UI を Canvas に GPU 描画し、IME に EditContext API を使う。WebGPU と EditContext API の両方が使えるとき選ばれる。
_Avoid_: GPU Mode（モードは描画先 Canvas を指す）

**HTML Mode**:
`hayate-adapter-web` のもう一つの動作モード。Hayate CSS をブラウザ CSS に直接マップし、レイアウトをブラウザ CSS エンジンに委ねる。WebGPU または EditContext API のいずれかが欠けるとき選ばれる、開発確認向けモード。
_Avoid_: フォールバック（劣化の含意を避けるため）、DOM Mode、absolutely-positioned div 方式

**Tsubame（燕）**:
JS/TS 向けのレンダラーターゲット基盤。Renderer Protocol（`IRenderer`）・DOM Renderer・Canvas Renderer を提供する層で、signal・コンポーネントモデル・スケジューラは持たない。Hayate とはアーキテクチャ上独立で、Hayate は Tsubame を知らない。
_Avoid_: signal ランタイム、フレームワーク、React hooks ベース、Virtual DOM

**tsubame-solid**:
Tsubame Adapter の一つ。SolidJS のランタイム（fine-grained signals 等）をそのまま使い、レンダリング先を Renderer Protocol に向け替える。
_Avoid_: Tsubame signal への依存

**tsubame-react**:
Tsubame Adapter の一つ。React の Fiber ランタイム（hooks・Suspense・Context 等）をそのまま使い、レンダリング先を Renderer Protocol に向け替える。
_Avoid_: Tsubame signal への依存、hooks 互換シム

**tsubame-vue**:
Tsubame Adapter の一つ。Vue のランタイム（`@vue/reactivity`・VDOM 等）をそのまま使い、`.vue` SFC のレンダリング先を Renderer Protocol に向け替える。
_Avoid_: @vue/reactivity を Tsubame signal に置き換える設計

**Tsubame Adapter**:
各フレームワークの既存ランタイムを Hayate（GPU Canvas）と DOM の両方にターゲットさせるブリッジ層。`tsubame-solid` / `tsubame-vue` / `tsubame-react` を指す（tsubame-svelte はスコープ外）。
_Avoid_: Solid-native, Vue-native, tsubame-svelte, signal 共有

**Renderer Protocol**:
Tsubame と Tsubame Adapter の境界インターフェース（`IRenderer`）。element 作成・ツリー操作・スタイル設定・イベント購読を抽象化し、DOM Renderer と Canvas Renderer の二実装を持つ。element property は閉じた typed 語彙で、未知 property 名はエラー。
_Avoid_: Host Interface, Host Config, Element Driver、untyped な setProperty で任意 HTML 属性を通す設計

**意味論パリティ（Semantics Parity）**:
Renderer Protocol のスタイル・イベント語彙は名前だけでなく**意味論ごと**契約であるという原則。継承・スクロール連鎖・フォント合成等の挙動は全レンダラーで同一でなければならず、Hayate CSS の定義が正準、DOM 系レンダラーがブラウザ既定挙動を抑制・補完して合わせる。
_Avoid_: 「DOM で動けば OK」、レンダラーごとの方言、語彙＝プロパティ名リストという理解

**DOM Renderer**:
Renderer Protocol 実装の一つ。Signal が DOM を直接操作する CSR 専用で、Hayate（WASM）を使わない。Hayate の HTML Mode とは別概念。
_Avoid_: Tsubame DOM Mode, SSG, SSR, ハイドレーション, Hayate HTML Mode

**Canvas Renderer**:
Renderer Protocol 実装の一つ。フレーム分の mutations を JS 側で積み 1 回/frame で Hayate（WASM）に渡す。Hayate の Canvas Mode と組み合わせて動く。
_Avoid_: Tsubame Canvas Mode, 個別 element_set_* 呼び出し

**Interaction Event**:
ポインタ・キーボード操作に起因する要素単位のイベント（`hover-enter` / `hover-leave` / `focus` / `blur` / `active-start` / `active-end` 等）。`:hover` / `:active` / `:focus` に応じたスタイル切替は Render Layer が解決する。
_Avoid_: Signal ベースの hover スタイル切替・Tsubame 経由の hover イベント購読

**Pseudo-state Style**:
Hayate CSS 内の `:hover` / `:active` / `:focus` ブロック。要素の base style に対する上書き。複数状態が同時成立したときの正準優先順は `focus < hover < active`（後勝ち）で、これは wire コード（hover=0 / active=1 / focus=2）とは別物。優先順は spec（`proto/spec/pseudo_states.json`）が正本で、Hayate core の `resolve_visual` と Tsubame DOM Renderer のルールバンド順が共にそこから生成・参照する（Semantics Parity）。
_Avoid_: pseudoStyle（別 prop）、Signal による hover スタイル切替、wire コード順を優先順と同一視する理解、DOM の挿入順（authoring 順）に優先順を委ねる設計

**Canonical Tree（正本ツリー）**:
描画・layout・hit-test の正本ツリー。Canvas/HTML 経路では Hayate の element ツリー、Tsubame DOM Renderer 経路ではブラウザ DOM が正本。`text` を含むすべての子を tree 上の element として表現する。経路ごとに実体は一つのみで、複製や mirror は持たない（`tsubame-solid` の Shadow Tree は構造専用の別索引であり、これ自体は正本ではない）。
_Avoid_: 描画正本を JS 側に複製する設計、Virtual DOM、仮想 TextNode、renderer 側 parent map を正本とする設計、Document Tree（旧称）

**Component**:
`.hybs` ファイル一つがコンポーネント一つに対応する。コンポーネント名はファイル名（拡張子除く）のアッパーキャメルケース。
_Avoid_: クラス、関数コンポーネント

**Template DSL**:
`.hybs` の `<template>` で使う言語非依存のマークアップ言語。タグ名は Hayate の `element-kind` に直接マップされ、HTML タグ名は使わない。
_Avoid_: HTML、JSX、テンプレートエンジン（Handlebars 等）

**Script Adapter**:
特定言語向けの Hayabusa SDK 実装。Signal・Computed・Effect・on_mount・on_destroy・prop・emit を当該言語のイディオムで提供する。一プロジェクトで使えるのは一つだけ。
_Avoid_: プラグイン、バインディング

**Prop**:
コンポーネントが外部から受け取る入力値。`<script>` 内で `prop("name")` の呼び出しにより宣言する。
_Avoid_: export

**Signal**:
Hayabusa のリアクティビティの基本単位。値の変化が依存する Computed・Effect に自動伝播する。
_Avoid_: State, Observable, Store

**Vite Plugin**:
TypeScript 向け Phase 1 のビルド統合形式。`vite.config.ts` に `hayabusa()` プラグインを追加して `.hybs` のコンパイルを有効化する。
_Avoid_: hayabusa CLI

**Hot Reload**:
ファイル変更を手動リロードなしに反映する仕組み。`<template>` / `<style>` の変更は全言語で即時反映、`<script>` の変更は言語による。
_Avoid_: HMR（Hot Module Replacement）

**Router**:
Hayabusa が提供する URL ベースのナビゲーション管理。現在の URL に対応するコンポーネントをレンダリングする。
_Avoid_: ページ遷移ライブラリ

**Store**:
コンポーネントをまたいで共有されるリアクティブ状態。単一コンポーネント内の Signal と異なり、アプリ全体またはサブツリーで参照できる状態の器。
_Avoid_: Signal（単一コンポーネントスコープ）、Redux Store

**Resource**:
非同期データ取得をリアクティブシステムに統合する仕組み。loading / error / data の各状態を Signal で表現する。
_Avoid_: Promise、async/await

**Scene Graph**:
Hayate 内部の保持型描画グラフ。描画オブジェクト間の親子・描画順序・transform / clip 関係を表す。
_Avoid_: Virtual DOM, Component Tree

**Scroll Offset**:
`scroll-view` element のスクロール位置（x, y）。基本 offset は Element Document Runtime が保持し、慣性・スナップ・rubber-band 等の物理演算は Platform Adapter が担う。`scroll` イベントはアプリ通知専用。
_Avoid_: Hayate が scroll 状態を一切持たない設計、物理演算を上位層が持つ設計、StyleProp::ScrollOffset

**Z-Order**:
React Native 方式の描画順序制御。同一 parent 内の兄弟間でのみ有効で、デフォルトは document order（後勝ち）、`z-index` で上書きする。CSS stacking context は持たない。
_Avoid_: NodeKind::Layer、グローバル z-index 順序

**Transform Group**:
Scene Graph の Node 種別の一つ。affine 変換行列を保持し子 Node 群に GPU 側で適用する。layout 再計算なしにサブツリー全体を変換でき、アニメーションの基盤となる。
_Avoid_: StyleProp::Transform（座標焼き込み方式）

**Node**:
Raw Layer が管理する描画プリミティブの最小単位。`rect` / `text-run` / `image` / `clip` / `layer` 等、GPU が直接処理できる型のみ存在する。
_Avoid_: Element（Element Layer の element と混同するため）, Component, Widget

**NodeId**:
Hayate が払い出す不透明なハンドル（generational arena）。上位層は entity↔NodeId のマッピングを自身で管理する。
_Avoid_: Entity ID

**Backend**:
GPU API 抽象層。Hayate は wgpu を唯一の Backend とし、wgpu が Vulkan / Metal / DX12 / WebGPU への変換を担う。
_Avoid_: Renderer, Driver

**Retained**:
Scene Graph が前フレームの状態を保持し、上位層は変更のあった Node のみ通知する方式。対義語は Immediate（毎フレーム全再構築）。Hayate は Retained を採用する。

**Glyph Atlas**:
レンダリング済みグリフを格納する GPU テクスチャ。

**AccessKit**:
プラットフォームの AT（Assistive Technology）へアクセシビリティツリーを報告するクロスプラットフォーム Rust ライブラリ。Hayate Core がツリーを生成し、Platform Adapter が AccessKit のプラットフォーム実装を呼んで AT に報告する。
_Avoid_: アクセシビリティ API、スクリーンリーダー
