# Hayate / Hayabusa

**Hayate（疾風）** は、アプリケーション UI のための**命令型・保持型・GPU ネイティブな UI 基盤**である。
**Hayabusa（隼）** は、Hayate の上で動く **Signal ベース SFC フレームワーク**である。

Hayate は UI フレームワークではない。状態管理でもない。Reconciler でもない。Component tree でもない。

Hayate の**外部公開サーフェスは Element Layer ベースの proto 契約の一つだけ**である（ADR-0049/0072）。上位層は Element Layer に element を作成し・スタイルを設定し・ツリーを組み立てる。Hayate 内部でレイアウト計算とスタイル解決を行い、**内部の Raw Layer**（絶対座標・GPUプリミティブの lowering target）のコマンド列に変換して GPU に送る。Raw Layer は内部実装であり外部公開しない（ADR-0072）。

DOM 互換は設計目標に含まない。

## Language

**Hayate（疾風）**:
命令型・保持型・GPU ネイティブな UI 基盤。**外部公開は Element Layer ベースの proto 契約のみ**（Raw Layer は内部の lowering target で非公開、ADR-0072）。内部でレイアウト・スタイル解決・レンダリングを担う。
_Avoid_: フレームワーク、ライブラリ、レンダラー単体

**Hayabusa（隼）**:
Hayate の Element Layer 上に構築された Signal ベースの SFC（Single-File Component）フレームワーク。`.hybs` ファイル形式を採用し、テンプレートとスタイルは言語非依存の Hayabusa DSL で記述する。スクリプト層はプロジェクト単位で選択された単一言語（TypeScript / Rust / Python 等）で記述され、言語アダプタ経由で Signal・Computed・Effect 等のリアクティブプリミティブを提供する。Hayate コアは Hayabusa の存在を知らない。
_Avoid_: Hayate の別名、エンジン、Rust 専用フレームワーク

**Element Layer（要素層）**:
Hayate の上位 WIT インターフェース。element tree の作成・Hayate CSS スタイルの設定・ツリー組み立てを受け付け、内部でレイアウト計算（Taffy）とスタイル解決を行い Raw Layer に渡す。Hayabusa および他言語 SDK はこの層を使う。
_Avoid_: 上位 API、UI 層、Scene Layer

**Element（要素）**:
Element Layer が受け付ける UI の構成単位。React Native 語彙を採用し、`view` / `text` / `image` / `button` / `text-input` / `scroll-view` を基本型とする。HTML タグ名（div / span / input 等）は使用しない。LLM の訓練データ上で React Native・SwiftUI・Jetpack Compose の三系統に共通する語彙であり、文脈なしでも意味が一意になる。
_Avoid_: div, span, section, p, h1〜h6（HTML 語彙全般）

**Inline Formatting Context（IFC）**:
`text` element が確立する inline 整形単位。**IFC root**（親が `text` でない `text`）は subtree（自身の `el.text` ＋ 子 `text`（inline text element） を document 順）を1つの Parley ranged layout として整形する Taffy leaf。**inline text element**（親が `text` の `text`）は Taffy box を持たず、親 IFC の styled range（font-family/size/weight/style/color/decoration）として合成される。シンタックスハイライト等の inline styled text を可能にする。Canvas Mode（Parley）が実装し、DOM Renderer / HTML Mode はブラウザ native IFC に委ねる。内部 seam は `InlineText`（ADR-0063）。
_Avoid_: leaf-string + 親 collapse モデル（ADR-0058 旧式、ADR-0063 が supersede）、inline text element を Taffy box とする設計

**Element クラス（block box / inline text element）**:
2b（ADR-0063）以降、element は2クラス。**block box**（`view`/`button`/`image`/`scroll-view`/`text-input` および IFC root の `text`）は Taffy ノードを1つ持つ。**inline text element**（IFC 内の子 `text`）は Taffy ノードを持たず親 IFC の range。
_Avoid_: 全 element = 1 Taffy box という旧前提

**Taffy Projection**:
`ElementTree` の block-box 部分集合から **lazy に派生する Taffy ツリー**（ADR-0064）。`ElementTree` が構造・データの唯一 owner で、Taffy は peer ではなく derived projection。構造 mutation は `ElementTree` と structure-dirty 集合のみ触り、Taffy は `LayoutPass::run` 冒頭で dirty-scoped に reconcile される。inline text element は投影に含まれない。RN の Fiber↔Yoga shadow tree、Flutter の Element↔RenderObject tree と同形。`TaffyProjection` seam が `TaffyTree` と ElementId↔NodeId マップを所有する。
_Avoid_: Taffy ツリーを owner とする設計（`TaffyTree<Element>`・inline text element を `Display::None`）、各 mutation site での手 sync、layout ツリーと document ツリーの 1:1 視

**テキスト継承（2チャネル）**:
ADR-0065 のテキストスタイル継承モデル。プロパティ名は CSS 準拠、適用セマンティクスは実物 Flutter 寄せ（LLM 予測可能性）。**(1) 通常 text スタイル**（`color`/`font-size`/`font-family`/`font-weight`/`font-style`/`text-decoration`）は **text→text（IFC 内）のみ**継承し block（`view` 等）を貫通しない。`InlineText` が解決。**(2) ambient Default Text Style**（`default-*`）は任意 element に設定でき block を貫通して既定値を供給する（Flutter `DefaultTextStyle` 相当）。解決順: 自身明示 → text 祖先継承 → ambient 既定 → ハード既定。
_Avoid_: CSS 式の全要素 font 継承（ADR-0047 旧式、`view` の font が text に漏れる）、RN の ambient 既定なし設計

**Default Text Style（ambient）**:
`default-font-family` / `default-font-size` / `default-font-weight` / `default-color`。block を貫通して top-down に降りる既定値専用チャネル（`scene_build.walk` が運ぶ）。text 要素が明示値も text 継承値も持たないときの fallback。nested 上書き可。app 全体の既定フォント/サイズ/色をここで置く。
_Avoid_: 通常 `font-family` 等を block 貫通させる設計、global と呼ぶ（nested 上書き可能な ambient であり真の global ではない）

**Hayate CSS**:
要素ごとのインラインスタイル宣言。レイアウトプロパティは Taffy の CSS サブセット、ビジュアルプロパティは CSS 名の Hayate 対応サブセット。要素ローカルの擬似状態（`:hover` / `:active` / `:focus`）を同一宣言内に nest でき、Render Layer がポインタ状態に応じて effective style を合成する。セレクタ・カスケード・スタイルシートは含まない。
_Avoid_: CSS（フル互換の含意）、CSS 風スタイル、Element Style

**Raw Layer（生座標層）**:
Hayate の**内部** lowering target。絶対座標・確定スタイル済みの描画コマンド（`SceneGraph` / `Node`）。Element Layer がレイアウト・スタイル解決の結果をこの層に変換して GPU に送る。**外部公開しない**（公開 Raw Layer 契約は ADR-0072 で棄却）。layout-free な game HUD / Infinite Canvas 等の需要は Element Layer の絶対座標 / transform で賄うか scope 外。
_Avoid_: 外部公開 API・公開 WIT/proto サーフェス（内部実装である）、Draw Layer

**WIT（WebAssembly Interface Types）**:
Hayate の公開 API の単一ソース。Element Layer と Raw Layer の両方を定義する。Web 向けビルドでは Wasm コンポーネントとしてコンパイルされ、ブラウザの Wasm ランタイム上で動作する。ネイティブ向けビルドでは wit-bindgen を通じてネイティブライブラリとしてコンパイルされ、Wasm ランタイムを必要としない。Hayate の WIT は原則として export のみで構成される。Hayate は上位層を知らず、上位層が Hayate をインポートして使う一方向依存が原則である。Hayabusa は Rust クレートとして hayate-core に直接依存するため WIT 境界を経由しない（ADR-0045）。WIT は Hayate の外部公開 API であり、Tsubame・他言語フレームワーク・サードパーティ SDK が使う契約として機能する。
_Avoid_: API 定義ファイル、インターフェース仕様書（言語間の実装契約として機能するため）

**Platform Adapter**:
IME 入力・クリップボード・raw 入力イベント変換を担い、Hayate Core とプラットフォームを仲介する層。プラットフォームごとに異なる実装を持つ（Web: Canvas Mode では EditContext API / HTML Mode では native DOM IME / macOS: TSM / Windows: TSF / Linux: IBus 等）。**IME plumbing は `ImeBridge` trait の裏に薄くラップするだけで、編集モデル（`EditState`：text_content/preedit/cursor、commit）は core が持つ（ADR-0069）。core が character bounds を `ImeBridge` へ供給し候補窓位置を満たす。**IME イベント（composition-start / composition-update / composition-end / commit-text）は Element Layer に届く。`text-input` が Element Layer の概念であり、IME 候補窓の位置計算に Taffy レイアウト結果が必要なため。Core は Platform Adapter を知らない。サーフェス生成とフレームタイミングは wgpu が担うため Adapter の責務に含まない。アクセシビリティ報告は AccessKit がコアに組み込まれるため Adapter の責務に含まない。
_Avoid_: Runtime, Host, Surface Adapter

**Canvas Mode**:
`hayate-adapter-web` の動作モードの一つ。Vello + wgpu（WebGPU）で全 UI を Canvas に GPU 描画し、IME に EditContext API を使用する。WebGPU（`navigator.gpu`）と EditContext API の両方が利用可能な場合に自動選択される。現時点では Chromium 系ブラウザが該当する。レイアウト（Taffy）・描画（Vello）・フォント（バンドルフォント）がネイティブビルドと同一コード・同一データを使用するため、アプリがフォントをバンドルする限りネイティブとのピクセル完全一致が保証される。

**HTML Mode**:
`hayate-adapter-web` の動作モードの一つ。WebGPU または EditContext API のいずれかが利用できない場合に自動選択される。Hayate CSS プロパティをブラウザの CSS プロパティに直接マッピングし、レイアウト計算はブラウザの CSS エンジンに委ねる（Taffy は経由しない）。Canvas Mode とはレンダリングパイプラインが異なるため、レイアウト結果の完全一致は保証されないが、開発時の UI 確認用途には十分な精度を持つ。IME はブラウザ native の動作に委ねる。モード選択はランタイム自動検出で行い、アプリ側は意識しない。
_Avoid_: フォールバック（劣化の含意を避けるため）、DOM Mode、absolutely-positioned div 方式

**Tsubame（燕）**:
JS/TS 向けの**レンダラーターゲット基盤**。フレームワークでも signal ランタイムでもなく、Renderer Protocol（`IRenderer`）・DOM Renderer・Canvas Renderer の3つを提供する層である。各 Tsubame Adapter は自身のフレームワーク固有のランタイム（SolidJS の signals / Vue の `@vue/reactivity` / React の Fiber）をそのまま持ち込み、レンダリング先を Tsubame の Renderer Protocol に向け替える。Tsubame は signal・コンポーネントモデル・スケジューラを持たない。Hayabusa・Hayate コアのいずれも Tsubame の存在を知らない。Hayate とは完全に独立した別リポジトリ（pure JS モノレポ）。
_Avoid_: signal ランタイム、フレームワーク、React hooks ベース、Virtual DOM を持たない（adapter が持ち込む）

**tsubame-solid**:
Tsubame Adapter の一つ。SolidJS の `solid-js/universal` カスタムレンダラー API を使い、SolidJS のランタイム（fine-grained signals / `onMount` / `onCleanup` 等）をそのまま維持しつつレンダリング先を Tsubame の Renderer Protocol に向け替える。SolidJS のエコシステム（Solid Router・SolidQuery 等）がそのまま動く。`.tsx` 形式・コンポーネント関数は一度だけ実行・Virtual DOM なし。
_Avoid_: Tsubame signal への依存（SolidJS 自身の signal を使う）

**tsubame-react**:
Tsubame Adapter の一つ。`react-reconciler` を使い、React の Fiber ランタイム（hooks・Suspense・Context 等）をそのまま維持しつつレンダリング先を Tsubame の Renderer Protocol に向け替える。TanStack Query・Zustand・Jotai 等の React エコシステムがそのまま動く。JSX/TSX 形式。既存の React コードを最小限の変更で Hayate（GPU Canvas）と DOM に対応させられる。
_Avoid_: Tsubame signal への依存（React 自身の Fiber ランタイムを使う）、hooks 互換シム（React 本体を使うため不要）

**tsubame-vue**:
Tsubame Adapter の一つ。`@vue/runtime-core` の `createRenderer()` API を使い、Vue のランタイム（`@vue/reactivity` の `ref`/`computed`/`watchEffect`・VDOM・コンポーネントライフサイクル）をそのまま維持しつつレンダリング先を Tsubame の Renderer Protocol に向け替える。Pinia・VueUse・VueRouter 等の Vue エコシステムがそのまま動く。`.vue` SFC 形式を採用し、`<template>` は `@vue/compiler-dom` のコードジェネレータ部分を差し替えて Renderer Protocol 呼び出しに変換する。`.vue` ファイル形式により Vue ユーザーおよび Svelte ユーザー（SFC 構文に親しみがある）が移行しやすい。
_Avoid_: @vue/reactivity を Tsubame signal に置き換える設計（Vue エコシステムが全滅するため）

**Tsubame Adapter**:
各フレームワークの既存ランタイムを Hayate（GPU Canvas）と DOM の両方にターゲットさせるブリッジ層。`tsubame-solid` / `tsubame-vue` / `tsubame-react` の3つを指す（tsubame-svelte はスコープ外。Svelte ユーザーには tsubame-vue を推奨）。各 adapter は自身のフレームワークのエコシステム（Pinia・TanStack Query 等の 3rd party ライブラリを含む）をそのまま維持し、レンダリング先を Tsubame の Renderer Protocol に向け替えるだけである。コンポーネントの UI ロジックは adapter をまたいで共有しない（記法が異なるため定義上不可能）。Tsubame リポジトリ内のモノレポ（`packages/renderer-protocol` / `packages/renderer-dom` / `packages/renderer-canvas` / `packages/solid` / `packages/vue` / `packages/react`）として管理される。Hayate リポジトリとは完全に独立した別リポジトリであり、結合点は `apply_mutations` の仕様のみ。
_Avoid_: Solid-native, Vue-native（既存プロジェクト名との衝突を避けるため）, tsubame-svelte, signal共有（各adapterが独自ランタイムを持つため）

**Renderer Protocol**:
Tsubame と Tsubame Adapter の間の境界インターフェース。element の作成・ツリー操作・スタイル設定・イベント購読を抽象化した仕様。TypeScript では `interface IRenderer` として定義される。DOM Renderer と Canvas Renderer の二つの実装を持つ。Tsubame Adapter はこのプロトコルを通じてのみレンダリングを行い、DOM か Canvas かを意識しない。element property は**閉じた typed 語彙**（`value`/`placeholder`/`disabled`/`src` 等の意味プロパティ。`aria` は first-class 経由）で、未知 property 名はエラー（任意 HTML 属性は禁止＝タグ禁止と同格、ADR-0071）。
_Avoid_: Host Interface, Host Config, Element Driver、untyped な setProperty で任意 HTML 属性を通す設計

**DOM Renderer**:
Renderer Protocol の実装の一つ。CSR（Client-Side Rendering）のみ。Signal が DOM を直接操作する。Hayate（WASM）を一切使用しない。JS→WASM 境界が存在しない。SSG・SSR・ハイドレーションは行わない。Hayate の HTML Mode とは別概念であり、Hayate が関与しない点が根本的に異なる。
_Avoid_: Tsubame DOM Mode, SSG, SSR, ハイドレーション, Hayate HTML Mode（Hayate 不使用のため）

**Canvas Renderer**:
Renderer Protocol の実装の一つ。JS 内でフレーム分の mutations を積み、`apply_mutations(batch)` で Hayate（WASM）に 1回/frame で渡す。JS→WASM 境界のコストを O(N) から O(1)/frame に削減する。Hayate の Canvas Mode（WebGPU GPU 描画）と組み合わせて動作する。Renderer Protocol を実装しているため、Tsubame Adapter はどちらの Renderer を使うかを意識しない。
_Avoid_: Tsubame Canvas Mode, 個別 element_set_* 呼び出し（Canvas Renderer では JS 側でバッチ化する）

**Interaction Event**:
ポインタやキーボード操作に起因する要素単位のイベント。`hover-enter` / `hover-leave` / `focus` / `blur` / `active-start` / `active-end` 等を含み、Hayate Element Layer が `poll-events()` で host に通知する。`:hover` / `:active` / `:focus` に応じた**スタイル切替**は Hayate Render Layer が解決する（ADR-0056）。Tsubame Adapter は `onHoverEnter` / `onHoverLeave` と Signal ベースのホバー状態を拒否する（ADR-0059）。
_Avoid_: Signal ベースの hover スタイル切替・Tsubame 経由の hover イベント購読（擬似スタイル宣言を使う）

**Pseudo-state Style**:
Hayate CSS 内の `:hover` / `:active` / `:focus` ブロック。要素の base style に対する上書きであり、Render Layer が解決する。
_Avoid_: pseudoStyle（別 prop）、Signal による hover スタイル切替

**Document Tree（文書ツリー）**:
**描画・layout・hit-test の正本**。Canvas/HTML 経路では Hayate `ElementTree`、Tsubame DOM Renderer 経路ではブラウザ DOM が正本。`text` を含むすべての子は tree 上の element として表現する（ADR-0058）。`tsubame-solid` は例外として、`solid-js/universal` が同期で読めるホストツリーを要求するため `TsubameNode` を**構造専用 reconcile index**（parent / 順序付き children のみ）として保持する（ADR-0062 が ADR-0057 を supersede）。これは描画正本の複製ではなく、batch 境界の向こうに正本を置く構成での reconcile 作業セット。`CanvasRenderer` / `DomRenderer` は構造を持たない（write-only）。
_Avoid_: 描画正本を JS 側に複製する設計、Virtual DOM（shadow tree は diff されないので VDOM ではない）、仮想 TextNode、renderer 側 parent map を正本とする設計

**Component**:
`.hybs` ファイル一つがコンポーネント一つに対応する。コンポーネント名はファイル名（拡張子除く）のアッパーキャメルケースで決まる（例: `MyButton.hybs` → `<MyButton>`）。名前の明示的な宣言は不要。`<script>` のトップレベルに宣言されたすべての名前は `<template>` から参照可能である。エクスポート宣言は不要。
_Avoid_: クラス、関数コンポーネント（`.hybs` ファイルそのものがコンポーネントの単位）

**Template DSL**:
`.hybs` の `<template>` セクション内で使う言語非依存のマークアップ言語。タグ名は Hayate の `element-kind`（`view` / `text` / `image` / `button` / `text-input` / `scroll-view`）に直接マップされる。HTML タグ名（`div` / `p` / `h1` 等）は使用しない。式は `{}` で囲まれた制限付き DSL で記述し、特定プログラミング言語の構文に依存しない。
_Avoid_: HTML、JSX、テンプレートエンジン（Handlebars 等）

**Script Adapter**:
特定言語向けの Hayabusa SDK 実装。Signal・Computed・Effect・on_mount・on_destroy・prop・emit の各プリミティブを当該言語のイディオムで提供する。Hayabusa Rust コアに言語ランタイムを埋め込む形で実装され（TypeScript: QuickJS、Python: PyO3、Rust: native）、Signal グラフの実体は Hayabusa Rust コアが保持する（ADR-0045）。一プロジェクトで使用できる Script Adapter は一つだけであり、`hayabusa.toml` の `[script] language` で宣言する。
_Avoid_: プラグイン、バインディング（WIT binding と混同するため）

**Prop**:
コンポーネントが外部から受け取る入力値。`<script>` 内で `prop("name")` 関数呼び出しにより宣言する（例: `const label = prop<string>("label")`）。コンパイラは `<script>` の AST を静的スキャンして `prop()` 呼び出しを検出し、コンポーネントの props インターフェースを確定する。`<template>` からは通常の識別子として参照できる。
_Avoid_: export（言語ごとのエクスポート構文はコンパイラの判定ルールが言語依存になるため使わない）

**Signal**:
Hayabusa のリアクティビティの基本単位。値の変化が依存する Computed・Effect に自動伝播する。グラフの追跡・伝播・スケジューリングは Hayabusa Rust コアが担い、各言語の Script Adapter は埋め込まれた言語ランタイム経由でこれを呼び出す（ADR-0045）。WIT は使わない。言語ごとの表記（Rust: `.get()` / TypeScript・Python: `.value`）は Script Adapter の薄いラッパーが提供する。テンプレートからは識別子のみで参照でき、コンパイラが言語別のアクセス形式に展開する。
_Avoid_: State, Observable, Store（Store は別の概念）

**Vite Plugin**:
TypeScript 向け Phase 1 のビルド統合形式。`vite.config.ts` に `hayabusa()` プラグインを追加することで `.hybs` ファイルのコンパイルが有効になる。`hayabusa.toml` はプラグインが参照する設定ファイルとして機能する。ユーザーは Vite を直接操作し、Hayabusa は Vite の変換パイプラインに乗る。Rust・Python 向け Phase 2 以降ではそれぞれの言語ツールチェーンに対応した統合形式を別途定義する。
_Avoid_: hayabusa CLI（TypeScript Phase 1 ではビルドの主役は Vite）

**Hot Reload**:
開発中にファイル変更を保存した際にブラウザを手動リロードせず変更を反映する仕組み。セクションごとに反映範囲が異なる。`<template>` と `<style>` の変更はすべての言語で即時反映される（Hayabusa コンパイラが処理し、Rust バイナリの再コンパイルを必要としない）。`<script>` の変更は言語によって異なり、TypeScript・Python は即時反映、Rust はフルリビルド後にリロードとなる。
_Avoid_: HMR（Hot Module Replacement）（モジュールシステムを持たない言語では意味をなさないため）

**Router**:
Hayabusa が提供する URL ベースのナビゲーション管理。現在の URL に対応するコンポーネントをレンダリングする責務を持つ。Signal ベースのリアクティブシステムと統合され、URL 変化がコンポーネントツリーに自動伝播する。
_Avoid_: ページ遷移ライブラリ（Hayabusa 組み込みのため）

**Store**:
コンポーネントをまたいで共有されるリアクティブ状態。単一コンポーネント内の Signal と異なり、アプリケーション全体またはサブツリーで参照可能な状態の器。Signal ランタイム上に構築される。
_Avoid_: Signal（単一コンポーネントスコープの Signal とは異なる）、Redux Store（実装モデルが異なる）

**Resource**:
非同期データ取得をリアクティブシステムに統合する仕組み。fetch・DB 問い合わせ等の非同期操作の結果を Signal として扱い、loading / error / data の各状態を Signal で表現する。
_Avoid_: Promise、async/await（Resource はリアクティブグラフの一部として機能する）

**Scene Graph**:
Hayate 内部の描画オブジェクト間の親子・描画順序・transform / clip 関係を表す保持型グラフ。z-order / transform 継承 / clip / hit-test / grouping のための補助構造。NodeId 指定で直接 mutation される実体オブジェクト群。
_Avoid_: Virtual DOM, Component Tree

**Scroll Offset**:
`scroll-view` Element のスクロール位置（x, y）。`Element Document Runtime` が基本 wheel 入力から offset を更新・保持する。Hayate は `scene_build` 時に offset 分だけ子を平行移動しクリップを適用する。`position: sticky` も同 offset を使って `scene_build` 内でクランプする。イナーシャ・スナップ・rubber-band 等の物理演算だけ上位層（Hayabusa / Framework）が任意で上書きする。
_Avoid_: Hayate が scroll 状態を一切持たない設計、StyleProp::ScrollOffset

**Z-Order**:
React Native 方式の描画順序制御。同一 parent 内の兄弟間でのみ有効。デフォルトは document order（後から追加された兄弟が上、いわゆる後勝ち）。`StyleProp::ZIndex(n)` はこのデフォルト順の上書きとして働き、数値が高い兄弟が前景に描画される。親の兄弟より前景に出す必要がある場合（モーダル・tooltip）は root 直下への配置で解決する。Hayate コアは CSS stacking context を持たない。Tsubame DOM Renderer は RN Web 現行方式（各 element に `position: relative` + デフォルト `zIndex: 0`）で同一セマンティクスを DOM 上にエミュレートする。ブラウザ CSS との完全一致は目標にしない。
_Avoid_: NodeKind::Layer、グローバル z-index 順序

**Transform Group**:
SceneGraph の Node 種別の一つ（`NodeKind::Group`）。affine 変換行列を保持し、子 Node 群に GPU 側で matrix を適用する。Vello の `push_transform()` / `pop()` に対応する。`StyleProp::Transform` として座標に焼き込む方式とは異なり、layout 再計算ゼロでサブツリー全体を変換できるため、アニメーションの基盤となる。
_Avoid_: StyleProp::Transform（座標焼き込み方式は layout 再計算が不要にならない）

**Node**:
Hayate の Raw Layer が管理する描画プリミティブの最小単位。`rect` / `text-run` / `image` / `clip` / `layer` 等、GPU が直接処理できる型のみ存在する。HTML の div/span や React Component とは異なる。
_Avoid_: Element（Element Layer の element と混同するため）, Component, Widget

**NodeId**:
Hayate が slotmap（generational arena）で払い出す不透明なハンドル。上位層は「どの entity が どの NodeId か」のマッピングを自身で管理する。削除済み Node への誤 mutation は generational check で検出される。
_Avoid_: Entity ID

**Backend**:
GPU API 抽象層。Hayate は wgpu を唯一の Backend として使用し、wgpu が Vulkan / Metal / DX12 / WebGPU（ブラウザ）への変換を担う。Hayate は独自の Backend 抽象を持たない。
_Avoid_: Renderer, Driver

**Retained**:
Scene Graph が前フレームの状態を保持し、上位層は変更のあった Node のみを通知する方式。対義語は Immediate（毎フレーム全 Node を再構築）。Hayate は Retained を採用する。

**Glyph Atlas**:
レンダリング済みグリフを格納する GPU テクスチャ。LRU でエビクションし、UV 座標でアドレス指定する。

**AccessKit**:
GUI アプリがプラットフォームの AT（Assistive Technology）へアクセシビリティツリーを報告するためのクロスプラットフォーム Rust ライブラリ。アプリ側は `TreeUpdate`（ツリー差分）を生成するだけでよく、Windows UIA / macOS NSAccessibility / AT-SPI / Web ARIA への橋渡しは AccessKit が担う。Hayate Core はツリーの生成責務を持ち、Platform Adapter が AccessKit のプラットフォーム実装を呼び出してシステムの AT に報告する。
_Avoid_: アクセシビリティ API、スクリーンリーダー（AT の一種に過ぎない）

## Example Dialogue

> 「Hayate は React の代替か？」
> → 「違う。Hayabusa が React 相当の役割を担う。Hayabusa が Signal diff を取り、変化分を Hayate Element Layer に流す。Hayate は受け取って描くだけ」

> 「他言語（Go・Zig・C）から Hayate を使えるか？」
> → 「使える。WIT から wit-bindgen で各言語のネイティブ SDK が自動生成される。Element Layer 経由でスタイル付き UI が作れるし、Raw Layer 経由で生座標を直接制御することもできる」

> 「Web とネイティブで挙動が変わるか？」
> → 「変わらない。WIT が単一ソースで両方にコンパイルされる。Platform Adapter の実装は異なる（Web Canvas Mode は EditContext API / Web HTML Mode は native DOM IME / macOS は TSM）が、Hayate Core は実装を知らない。品質は等階級」

> 「IME はどこが担うか？」
> → 「Platform Adapter が担う。WIT に IME インターフェース（composition-start / composition-update / composition-end / commit-text）を定義し、各プラットフォームの Adapter が実装する」
