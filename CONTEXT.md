# Hayate / Hayabusa

**Hayate（疾風）** は、アプリケーション UI のための命令型・保持型・GPU ネイティブな UI 基盤である。
**Hayabusa（隼）** は、Hayate の上で動く Signal ベース SFC フレームワークである。

Hayate は UI フレームワーク・状態管理・Reconciler・Component tree のいずれでもない。DOM 互換は設計目標に含まない。

> 語彙の正本。各語が**何であるか**を定義する。実装の仕組み・根拠・決定は `docs/spec/` と各 ADR に置き、ここには書かない。

## Language

**Hayate（疾風）**:
命令型・保持型・GPU ネイティブな UI 基盤。上位層が組み立てた element を受け取って描画する。外部公開サーフェスは Element Layer ただ一つで、wire（proto 契約・JS/Tsubame 経路）と in-process Rust（ADR-0045・Hayabusa 経路）の二 projection を持つ。
_Avoid_: フレームワーク、ライブラリ、レンダラー単体、proto 契約を唯一の公開サーフェスとする理解

**Hayabusa（隼）**:
Hayate の Element Layer 上に構築された Signal ベースの SFC（Single-File Component）フレームワーク。リアクティブランタイム（Signal グラフ・伝播・reconcile・スケジューリング）を Rust で**単独所有**する点が、各言語の既存ランタイムを再利用する Tsubame と対をなす（意図的な逆張り）。`.hybs` 形式で、テンプレートとスタイルは言語非依存の DSL、スクリプトは WASM にコンパイル可能な任意の単一言語で書く。JS/TS は Tsubame の領分であり Hayabusa の対象言語ではない。
_Avoid_: Hayate の別名、エンジン、Rust 専用フレームワーク、JS/TS 向けフレームワーク、Tsubame と同じく言語側ランタイムを再利用する設計

**Element Layer（要素層）**:
Hayate の唯一の外部公開サーフェス。element tree の作成・Hayate CSS の設定・ツリー組み立てを受け付ける上位の概念層。消費経路は二つの projection を持つ — JS/Tsubame Hayate Renderer は wire（proto 契約）projection で apply_mutations に符号化し、Hayabusa は Rust crate 依存（ADR-0045）の in-process projection で wire を介さず直接駆動する。後者は ElId＝ElementId の同一 identity を共有する。どちらの経路も同一の Element Layer 意味論を見る。
_Avoid_: 上位 API、UI 層、Scene Layer、Hayabusa も wire/proto 経由とする理解、経路ごとに別 API とする理解

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

**Style Channel（スタイルチャネル）**:
各 Hayate CSS プロパティが属する継承チャネルの分類。`text-local`（ch1）/ `ambient`（ch2）/ `none`（box-visual・layout）の三値。`proto/spec/style_tags.json` の `inherit` tag が正本で、レンダラー横断の text-channel 判定（どのプロパティを text 要素にのみ発行するか）はここから生成し、各レンダラーで再宣言しない。
_Avoid_: reach（再 lowering 距離は直交する別軸）、text-local を boolean 一値とし ambient を none に潰す理解

**Text-Local Carrier（text-local キャリア）**:
channel-1（text-local）スタイルを CSS として実際に載せる element-kind。`text` / `text_input` の二つで、`proto/spec/element_kinds.json` の `carriesTextLocal` tag が正本。block box はキャリアではなく、block への text-local プロパティは no-op（ADR-0002）。
_Avoid_: 全 element がテキストスタイルを受理する設計、DOM tag（span / input）で carrier を同定する理解

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
JS/TS 向けのレンダラーターゲット基盤。Renderer Protocol（`IRenderer`）・DOM Renderer・Hayate Renderer を提供する層で、signal・コンポーネントモデル・スケジューラは持たない。Hayate とはアーキテクチャ上独立で、Hayate は Tsubame を知らない。
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
Tsubame と Tsubame Adapter の境界インターフェース（`IRenderer`）。element 作成・ツリー操作・スタイル設定・イベント購読を抽象化し、DOM Renderer と Hayate Renderer の二実装を持つ。element property は閉じた typed 語彙で、未知 property 名はエラー。
_Avoid_: Host Interface, Host Config, Element Driver、untyped な setProperty で任意 HTML 属性を通す設計

**意味論パリティ（Semantics Parity）**:
Renderer Protocol のスタイル・イベント語彙は名前だけでなく**意味論ごと**契約であるという原則。継承・スクロール連鎖・フォント合成等の挙動は全レンダラーで同一でなければならず、Hayate CSS の定義が正準、DOM 系レンダラーがブラウザ既定挙動を抑制・補完して合わせる。
_Avoid_: 「DOM で動けば OK」、レンダラーごとの方言、語彙＝プロパティ名リストという理解

**DOM Renderer**:
Renderer Protocol 実装の一つ。Signal が DOM を直接操作する CSR 専用で、Hayate（WASM）を使わない。Hayate の HTML Mode とは別概念。
_Avoid_: Tsubame DOM Mode, SSG, SSR, ハイドレーション, Hayate HTML Mode

**Hayate Renderer**:
Renderer Protocol 実装の一つ。フレーム分の mutations を JS 側で積み 1 回/frame で Hayate（WASM）に渡す。Hayate の Canvas Mode と組み合わせて動く。DOM Renderer と対をなし、両者はターゲット（Hayate / DOM）で区別する。旧称 Canvas Renderer は HTML `<canvas>` と混同するためターゲット名へ改名した（Tsubame ADR-0011）。
_Avoid_: Canvas Renderer（旧称・ADR-0011）, Tsubame Canvas Mode, 個別 element_set_* 呼び出し

**Interaction Event**:
ポインタ・キーボード操作に起因する要素単位のイベント（`hover-enter` / `hover-leave` / `focus` / `blur` / `active-start` / `active-end` 等）。`:hover` / `:active` / `:focus` に応じたスタイル切替は Render Layer が解決する。
_Avoid_: Signal ベースの hover スタイル切替・Tsubame 経由の hover イベント購読

**Selection（選択）**:
アプリ全体で同時に一つだけ存在する有効な文字選択。anchor と focus を `(ElementId, byte offset)` で表し、document 順に正規化した連続範囲。Element Document Runtime が単独所有し、Selection Region の選択と text-input の選択は排他（新規選択で旧選択は解除）。単一キャレット（EditState の `cursor_byte_index`）は anchor=focus の縮退形。runtime 内では `Selection` を deep module 化して正規化・縮退・contains 境界 clamp の不変条件を interface の裏へ集約し、値の field は `Interaction` module が持つ（read 経路は interface 越しに borrow）。これは runtime 内の再配置であり「runtime が単独所有」は不変（ADR-0122）。
_Avoid_: 領域ごとに独立した複数の同時選択、cursor を selection と別概念とする理解

**Selection Region（選択領域＝`user-select: contains` 境界）**:
選択がその外へ広がるのを止める、opt-in の封じ込め境界。`user-select: contains` を持つ block box が確立し（Flutter の SelectionArea 相当）、選択はこの境界を越えない。nested は最寄り祖先が有効。**既定では境界は無く**、選択は document 順に要素をまたいで自由に広がる（ブラウザ `user-select` パリティ・ADR-0108）。何が選択可能かは element-kind の UA 既定（text / view / scroll-view = 選択可、button / image = 不可）と明示 `user-select`（`text | none | contains`）で決まり、text-input は境界に依らず常に編集選択可能。
_Avoid_: 既定で領域内にのみ選択可とする opt-in 設計（廃止された `selectable` boolean）、選択は単一要素に閉じるという理解、`contains` 境界を既定とみなす理解、専用 element-kind

**Selection Chrome（選択 chrome）**:
有効な選択に対して core が SceneGraph に描く視覚要素 — highlight tint・ドラッグ handle・フローティングツールバー（拡大鏡は将来）。core が一度だけ描画する。**highlight tint は browser（Chromium `::selection`）をお手本に寄せ**、handle・ツールバー・拡大鏡は **browser に無い概念なので Android-native をお手本**にする（Canvas の視覚お手本は DOM＝ADR-0102）。DOM / HTML 経路ではブラウザネイティブの選択描画に委ねる（Selection Region の意味論はパリティ対象）。
_Avoid_: OS ネイティブ選択 UI を Platform Adapter ごとに再実装する設計、レンダラーごとの chrome 方言、tint まで Material 固定とし browser 寄せを否定する理解、chrome の見た目を意味論パリティの対象とする理解

**Scrollbar Chrome（スクロールバー chrome）**:
`scroll-view` のスクロール位置を表す視覚要素で、core が overlay（レイアウト非予約）で描く。Selection Chrome の姉妹概念で Pointer Modality で形態が分岐する — Mouse/Pen は Chromium をお手本にした操作可能なスクロールバー（thumb ドラッグ・track クリックで Scroll Offset を動かす）、Touch は Android-native をお手本にしたスクロール中のみ出る非操作の transient indicator。視覚お手本は DOM（ADR-0102）で、操作は Scroll Offset レベルでパリティする（ADR-0110）。
_Avoid_: classic（gutter 予約）スクロールバー設計、modality 非依存の単一形態、Canvas で非描画＝by-design とする理解、見た目を意味論パリティの対象とする理解

**EditIntent（編集インテント）**:
text-input の編集を表す閉じたコマンド語彙（move / extend / delete を境界・方向・粒度で、加えて select-all / copy / cut / paste）。Element Document Runtime が `EditState` に適用する唯一の編集シームで、キーレベルの編集挙動は Chromium `<input>` / `<textarea>` を正準とする。Platform Adapter が OS キーバインドをこの語彙へ翻訳し（core は OS を知らない）、Canvas 経路では proto 契約に載る。
_Avoid_: キー文字列を直接解釈する設計、OS keymap を core に持たせる理解、EditState への個別メソッド増殖

**Pointer Modality（ポインタ種別 / PointerKind）**:
入力ポインタが Mouse / Touch / Pen のいずれかという軸。Element Document Runtime がインタラクション単位で保持し、選択 chrome（handle / toolbar）の表示と blur 時の選択ライフサイクル（Mouse/Pen=非表示＋範囲記憶＋再フォーカス復活 / Touch=caret へ collapse＋chrome dismiss）を分岐させる。`:focus-visible` 用の Pointer/Keyboard（Input Modality）とは並立する別軸。
_Avoid_: 起動時の form-factor 固定、long-press から touch を推定する設計、Input Modality（Pointer/Keyboard）との混同

**Pseudo-state Style**:
Hayate CSS 内の `:hover` / `:active` / `:focus` ブロック。要素の base style に対する上書き。複数状態が同時成立したときの正準優先順は `focus < hover < active`（後勝ち）で、これは wire コード（hover=0 / active=1 / focus=2）とは別物。優先順は spec（`proto/spec/pseudo_states.json`）が正本で、Hayate core の `resolve_visual` と Tsubame DOM Renderer のルールバンド順が共にそこから生成・参照する（Semantics Parity）。
_Avoid_: pseudoStyle（別 prop）、Signal による hover スタイル切替、wire コード順を優先順と同一視する理解、DOM の挿入順（authoring 順）に優先順を委ねる設計

**Transition（visual 変化の補間）**:
要素の effective visual が変化したとき、変化前の**画面上の見た目**（補間中ならその時点の途中値）から新しい target へ連続値プロパティを補間するアニメーション。トリガは effective style 解決シーム（`resolve_effective`・ADR-0067）の差分であり、擬似状態切替・`setStyle`・継承変化を区別しない（ブラウザ/Blink の computed-style 差分と同型）。`transition-duration`（ms）/ `transition-timing`（`ease` / `linear` / `ease-in` / `ease-out` / `ease-in-out`）を Hayate CSS の visual プロパティとして持ち、Render Layer が `render(timestamp_ms)` のフレームループで進める（カーソル点滅と同じ時間駆動 `visual_dirty` を再利用）。補間対象は連続値（`background-color` / `border-color` / `text-color` / `opacity` / `border-radius` / `border-width` / `box-shadow`）のみで、enum・離散値は target を即時採用。`box-shadow` は CSS 準拠で、変化前後の shadow リスト長が等しく各 `inset` フラグが一致するときのみ位置ごとに offset / blur / spread / color を補間し、長さまたは inset 不一致のときは離散（即時 target 採用）。state は要素×プロパティ単位。`transition-duration: 0`（未指定）は即時切替。
_Avoid_: 擬似状態切替のみをトリガとする理解（旧スコープ・ADR-0089）、wire 入口の生 mutation を diff する設計、setStyle を恒久的に即時固定とする理解、Signal によるアニメーション、layout/text への補間

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
特定言語向けの Hayabusa SDK 実装。Signal・Computed・Effect・on_mount・on_destroy・prop・emit を当該言語のイディオムで*表面*として提供するが、リアクティビティの実体は Rust ランタイムが所有し、Adapter はランタイムへの薄いエンコーダ＋ハンドラ本体の登録に徹する。言語ごとに Signal 意味論を実装し直さない。一プロジェクトで使えるのは一つだけ。
_Avoid_: プラグイン、バインディング、言語ごとにリアクティビティを再実装する設計

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
コンポーネントをまたいで共有されるリアクティブ状態。単一コンポーネント内の Signal と異なり、アプリ全体、または特定コンポーネントとその子孫コンポーネントで参照できる状態の器。
_Avoid_: Signal（単一コンポーネントスコープ）、Redux Store

**Resource**:
非同期データ取得をリアクティブシステムに統合する仕組み。loading / error / data の各状態を Signal で表現する。
_Avoid_: Promise、async/await

**Scene Graph**:
Hayate 内部の保持型描画グラフ。描画オブジェクト間の親子・描画順序・transform / clip 関係を表す。
_Avoid_: Virtual DOM, Component Tree

**Scroll Offset**:
`scroll-view` element のスクロール位置（x, y）。Element Document Runtime が基本 offset を単独所有する。慣性・スナップ・rubber-band 等のスクロール物理は **Scroll Physics Profile** として Hayate Core が所有し（ADR-0046 を supersede）、Platform Adapter はフレーム駆動（毎フレーム step を進める）と platform 識別の供給に徹する。`scroll` イベントはアプリ通知専用。
_Avoid_: 物理演算を Platform Adapter が単独実装する設計（旧 ADR-0046）、Hayate が scroll 状態を一切持たない設計、StyleProp::ScrollOffset

**Scroll Chaining（スクロール連鎖）**:
ネストした `scroll-view` のホイール挙動。最寄り祖先 ScrollView から軸ごとにデルタを消費し、clamp で消費しきれなかった残デルタを次の祖先 ScrollView へルートまで伝播する、ブラウザ準拠の意味論。Hayate の仕様であり、DOM 系レンダラーはブラウザ既定のまま一致する（意味論パリティ）。
_Avoid_: 内側スクローラーで打ち止めにする設計、残デルタを捨てる設計、chaining をレンダラー方言とする理解（opt-out の `overscroll-behavior` 語彙は将来）

**Scroll Gesture（スクロールジェスチャ認識）**:
raw ポインタ列を「タップか scroll か」「どの `scroll-view` を掴んだか」「いま適用すべき 1:1 follow デルタ」へ分類する、物理に先行する純粋な意図分類。Hayate Core が所有し、slop 閾値などの tunable 値は Platform Adapter が引数で供給する。物理（慣性・rubber-band）とは別軸で、scroll を始めるか否かまでを決める。
_Avoid_: 物理演算と混同する理解、slop 値を core 固定にする設計、Platform Adapter ごとに状態機械を再実装する設計

**Scroll Physics Profile（スクロール物理プロファイル）**:
スクロール物理（フリング減衰曲線・overscroll の挙動・spring back）の「感触」を選ぶ閉じた語彙。`auto` / `ios` / `android` の三値で、`auto` は Platform Adapter が渡す platform 識別から各 OS 相当の感触へ解決する（完成形）。iOS 風（指数減衰＋sigmoid rubber-band）と Android 風（OverScroller の spline＋Material stretch）は別アルゴリズムで、いずれも Hayate Core が所有する。現状は `auto` のみ公開し、明示上書きは将来。
_Avoid_: 物理を Platform Adapter が所有する設計（ADR-0046 を supersede）、定数差だけで両 OS を表す理解（アルゴリズム自体が別）、起動時 form-factor 固定、Scroll Gesture（意図分類）との混同

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
Scene Graph が前フレームの lowering 結果（element anchor とその subtree）を保持し、dirty 集合に載った element のみ再 lowering する方式。クリーンフレームでは re-lowering walk ゼロ。対義語は Immediate（毎フレーム全再構築）。Hayate は Retained incremental lowering を採用する（ADR-0086）。
_Avoid_: 毎フレーム allocate-and-discard する Immediate lowering

**Element Anchor（要素アンカー）**:
element ごとの retained scene identity を担う構造専用 `NodeKind::ElementAnchor`。transform を持たず、子 element は親 anchor の子リストで子 anchor を参照する。1 element の border / rect / text run / Group / Clip は anchor 配下に lowering される。
_Avoid_: Group（transform 用語との混同）

**Glyph Atlas**:
レンダリング済みグリフを格納する GPU テクスチャ。

**Font Synthesis（フォント合成）**:
要求された `font-weight` / `font-style` が実フェイスや variable 軸で表現できないとき、ブラウザ準拠で見た目を合成する仕組み。faux italic はグリフランの skew（約14度）、faux bold は embolden で表す。意味論パリティのため既定 ON で、Canvas / DOM 双方が同一の合成挙動を示す。
_Avoid_: 表現できない指定を no-op にする設計、italic 実フェイスのバンドル前提、レンダラーごとに合成有無が異なる設計、font-synthesis を常時オフ前提とする説明（opt-out 語彙は将来）

**AccessKit**:
プラットフォームの AT（Assistive Technology）と**双方向**にやり取りするクロスプラットフォーム Rust ライブラリ。outbound では Hayate Core がアクセシビリティツリーを生成し Platform Adapter が AT に報告する。inbound では AT の Accessibility Action を Core が受けて既存の runtime 操作へ写像する。
_Avoid_: アクセシビリティ API、スクリーンリーダー、outbound 片方向（ツリー報告のみ）の理解

**Accessibility Action（アクセシビリティアクション）**:
AT が要素に対して要求する inbound の意味論操作（activate / focus / set value / set selection / scroll into view 等）。Core が単独で既存の runtime intent（`Click` イベント・focus 遷移・統一 Selection・Scroll Offset 等）へ写像し、ポインタやキーの**合成入力は経由しない**（Flutter の semantic action と同型：ジェスチャの replay ではなく intent の直接駆動）。Platform Adapter は OS の AT 配管として要求を Core へ橋渡しするだけ。
_Avoid_: 合成ポインタ/キーイベントの replay、Platform Adapter ごとのアクション解釈、Interaction Event（pointer/keyboard 由来の outbound 通知）との混同
