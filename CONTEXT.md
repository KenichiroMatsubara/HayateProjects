# Hayate / Hayabusa

**Hayate（疾風）** は、アプリケーション UI のための**命令型・保持型・GPU ネイティブな UI 基盤**である。
**Hayabusa（隼）** は、Hayate の上で動く **Signal 型 Rust フレームワーク**である。

Hayate は UI フレームワークではない。状態管理でもない。Reconciler でもない。Component tree でもない。

Hayate が提供するのは、Element Layer（element tree + CSS 風スタイル解決）と Raw Layer（絶対座標・GPUプリミティブ）の二層 WIT インターフェースである。上位層は Element Layer に element を作成し・スタイルを設定し・ツリーを組み立てる。Hayate 内部でレイアウト計算とスタイル解決を行い、Raw Layer のコマンド列に変換して GPU に送る。

DOM 互換は設計目標に含まない。

## Language

**Hayate（疾風）**:
命令型・保持型・GPU ネイティブな UI 基盤。Element Layer と Raw Layer の二層 WIT インターフェースを公開し、内部でレイアウト・スタイル解決・レンダリングを担う。
_Avoid_: フレームワーク、ライブラリ、レンダラー単体

**Hayabusa（隼）**:
Hayate の Element Layer 上に構築された Signal 型 Rust フレームワーク。`view!` マクロ・`#[component]`・Signal / Memo / Effect・Router・Store・Resource を提供する。Hayate の上を走る存在。
_Avoid_: Hayate の別名、エンジン

**Element Layer（要素層）**:
Hayate の上位 WIT インターフェース。element tree の作成・Hayate CSS スタイルの設定・ツリー組み立てを受け付け、内部でレイアウト計算（Taffy）とスタイル解決を行い Raw Layer に渡す。Hayabusa および他言語 SDK はこの層を使う。
_Avoid_: 上位 API、UI 層、Scene Layer

**Element（要素）**:
Element Layer が受け付ける UI の構成単位。React Native 語彙を採用し、`view` / `text` / `image` / `button` / `text-input` / `scroll-view` を基本型とする。HTML タグ名（div / span / input 等）は使用しない。LLM の訓練データ上で React Native・SwiftUI・Jetpack Compose の三系統に共通する語彙であり、文脈なしでも意味が一意になる。
_Avoid_: div, span, section, p, h1〜h6（HTML 語彙全般）

**Hayate CSS**:
Hayate 固有のスタイルシステム。レイアウトプロパティ（display / gap / align-items / grid-template-columns 等）は Taffy の CSS Flexbox / Grid / Block 実装を仕様書とする。ビジュアルプロパティ（color / background-color / border-radius / opacity 等）は CSS プロパティ名を踏襲しつつ Hayate が対応サブセットを定義する。CSS 互換実装ではなく、CSS 命名を採用した Hayate 固有の仕様である。
_Avoid_: CSS、CSS 風スタイル、Element Style

**Raw Layer（生座標層）**:
Hayate の下位 WIT インターフェース。絶対座標・確定スタイル済みの描画コマンドを直接受け付ける。レイアウトを自前で計算するユーザー（ゲーム HUD・Infinite Canvas 等）向けに公開する。Element Layer は内部でこの層に変換して使う。
_Avoid_: 内部 API（WIT で外部公開されるため）、Draw Layer

**WIT（WebAssembly Interface Types）**:
Hayate の公開 API の単一ソース。Element Layer と Raw Layer の両方を定義する。Web 向けビルドでは Wasm コンポーネントとしてコンパイルされ、ブラウザの Wasm ランタイム上で動作する。ネイティブ向けビルドでは wit-bindgen を通じてネイティブライブラリとしてコンパイルされ、Wasm ランタイムを必要としない。**Hayate と Hayabusa（別リポジトリ）の物理的な境界線**でもある。Hayabusa は Hayate の WIT をインポートして wit-bindgen で binding を生成し、その binding 経由でのみ Hayate を呼び出す。WIT の外側は Hayate の責務ではない。Hayate の WIT は原則として export のみで構成される。Hayate は上位層を知らず、上位層が Hayate をインポートして使う一方向依存が原則である。
_Avoid_: API 定義ファイル、インターフェース仕様書（言語間の実装契約として機能するため）

**Platform Adapter**:
IME 入力・クリップボード・raw 入力イベント変換を担い、Hayate Core とプラットフォームを仲介する層。プラットフォームごとに異なる実装を持つ（Web: Canvas Mode では EditContext API / HTML Mode では native DOM IME / macOS: TSM / Windows: TSF / Linux: IBus 等）。IME イベント（composition-start / composition-update / composition-end / commit-text）は Element Layer に届く。`text-input` が Element Layer の概念であり、IME 候補窓の位置計算に Taffy レイアウト結果が必要なため。Core は Platform Adapter を知らない。サーフェス生成とフレームタイミングは wgpu が担うため Adapter の責務に含まない。アクセシビリティ報告は AccessKit がコアに組み込まれるため Adapter の責務に含まない。
_Avoid_: Runtime, Host, Surface Adapter

**Canvas Mode**:
`hayate-adapter-web` の動作モードの一つ。Vello + wgpu（WebGPU）で全 UI を Canvas に GPU 描画し、IME に EditContext API を使用する。WebGPU（`navigator.gpu`）と EditContext API の両方が利用可能な場合に自動選択される。現時点では Chromium 系ブラウザが該当する。

**HTML Mode**:
`hayate-adapter-web` の動作モードの一つ。WebGPU または EditContext API のいずれかが利用できない場合に自動選択される。Element Layer → Taffy → Raw Layer という `hayate-core` の統一パイプラインを通り、Raw Layer の絶対座標出力を absolutely-positioned な HTML 要素（`div` 等）にマッピングして描画する。Canvas Mode と同一のコードパスを経由し、最終的な描画先だけが異なる（DOM vs GPU）。IME はブラウザ native の動作に委ねる。モード選択はランタイム自動検出で行い、アプリ側は意識しない。
_Avoid_: フォールバック（劣化の含意を避けるため）、DOM Mode、native CSS モード

**Interaction Event**:
ポインタやキーボード操作に起因する要素単位のイベント。`hover-enter` / `hover-leave` / `focus` / `blur` / `active-start` / `active-end` 等を含み、`poll-events()` で上位層に通知される。Hayate はイベントを通知するだけであり、インタラクション状態に応じたスタイル切り替えは上位層（Hayabusa の Signal / Effect）の責務。Hayate は「ホバー中スタイル」という概念を持たない。
_Avoid_: :hover スタイル、状態付きスタイル、CSS 擬似クラス

**Signal**:
Hayabusa のリアクティビティの基本単位。アリーナ型実装により `Copy` 可能なトークンとして提供され、所有権問題を回避する。Signal の値変化は依存する Memo・Effect・View に自動伝播する。
_Avoid_: State, Observable, Store（Store は別の概念）

**Scene Graph**:
Hayate 内部の描画オブジェクト間の親子・描画順序・transform / clip 関係を表す保持型グラフ。z-order / transform 継承 / clip / hit-test / grouping のための補助構造。NodeId 指定で直接 mutation される実体オブジェクト群。
_Avoid_: Virtual DOM, Component Tree

**Scroll Offset**:
`scroll-view` Element のスクロール位置（x, y）。Hayate は保持せず、上位層（Hayabusa）が `poll-events()` の scroll イベントから delta を積算して管理する。毎フレーム `element_set_scroll_offset(id, x, y)` で Hayate に渡し、Hayate は `scene_build` 時に子要素の座標をオフセット分だけ平行移動しクリップ矩形を適用する。`position: sticky` も同 scroll offset を使って `scene_build` 内でクランプ計算するため、Hayate の責務に含む。イナーシャ・スナップ等の物理演算は Hayabusa の責務。
_Avoid_: Hayate がスクロール状態を持つ設計、StyleProp::ScrollOffset

**Z-Order**:
SceneGraph の描画順序制御。同一 parent 内で `StyleProp::ZIndex(n)` が高い子ほど後に walk され、前景に描画される（painter's algorithm）。「親の兄弟より前景に出る」ケース（モーダル・tooltip）はアプリ側が root 直下に要素を配置することで解決し、CSS stacking context 相当の概念は Hayate に持ち込まない。
_Avoid_: NodeKind::Layer、stacking context

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

## Example Dialogue

> 「Hayate は React の代替か？」
> → 「違う。Hayabusa が React 相当の役割を担う。Hayabusa が Signal diff を取り、変化分を Hayate Element Layer に流す。Hayate は受け取って描くだけ」

> 「他言語（Go・Zig・C）から Hayate を使えるか？」
> → 「使える。WIT から wit-bindgen で各言語のネイティブ SDK が自動生成される。Element Layer 経由でスタイル付き UI が作れるし、Raw Layer 経由で生座標を直接制御することもできる」

> 「Web とネイティブで挙動が変わるか？」
> → 「変わらない。WIT が単一ソースで両方にコンパイルされる。Platform Adapter の実装は異なる（Web Canvas Mode は EditContext API / Web HTML Mode は native DOM IME / macOS は TSM）が、Hayate Core は実装を知らない。品質は等階級」

> 「IME はどこが担うか？」
> → 「Platform Adapter が担う。WIT に IME インターフェース（composition-start / composition-update / composition-end / commit-text）を定義し、各プラットフォームの Adapter が実装する」
