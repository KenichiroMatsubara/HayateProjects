# NewDOM

NewDOM は、アプリケーション UI のための**命令型・保持型・GPU ネイティブな描画オブジェクトモデル**である。

NewDOM は UI フレームワークではない。状態管理でもない。Reconciler でもない。Component tree でもない。

NewDOM が提供するのは、安定した NodeId を持つ描画オブジェクト群であり、上位層はそれらを直接 create / update / move / destroy する。Tree・Signal・VDOM・ECS・Layout Engine は、NewDOM に mutation を流し込む **producer** であって、NewDOM コアそのものではない。

## Language

**NewDOM**:
命令型・保持型・GPU ネイティブな描画オブジェクトモデル。NodeId で識別される描画オブジェクト群を保持し、上位層からの直接 mutation（create / update / move / destroy）を受け取り、render command を生成して GPU に送る。
_Avoid_: フレームワーク、ライブラリ、レンダラー単体、UI エンジン

**Substrate**:
NewDOM そのものを指す別名。UI フレームワークの下層、GPU の上層に位置する基盤層。
_Avoid_: エンジン、フレームワーク、ランタイム

**Node**:
NewDOM が管理する描画オブジェクトの最小単位。`ND_NODE_RECT` / `ND_NODE_TEXT` 等 GPU が直接理解できる型のみ存在する。HTML の div/span や React Component とは異なる。
_Avoid_: Element, Component, Widget

**NodeId**:
NewDOM が slotmap（generational arena）で払い出す不透明なハンドル。C ABI では `uint64_t` として公開する。上位層は「どの entity が どの NodeId か」のマッピングを自身で管理する。削除済み Node への誤 update は generational check で検出される。
_Avoid_: Entity ID

**Scene Graph**:
NewDOM 内部の描画オブジェクト間の**親子・描画順序・transform / clip 関係**を表す保持型グラフ。React / Vue の UI Tree や Virtual DOM ではなく、NodeId 指定で直接 mutation される実体オブジェクト群である。Tree 構造は z-order / transform 継承 / clip / hit-test / grouping / layering のための補助構造であり、NewDOM の本質ではない。
_Avoid_: Virtual DOM, Element Tree, Component Tree

**Retained**:
Scene Graph が前フレームの状態を保持し、上位層は変更のあった Node のみを通知する方式。対義語は Immediate（毎フレーム全 Node を再構築）。

**Immediate**:
NewDOM が採用しない方式。毎フレーム全 Node を CPU で走査するコストと、テキスト shaping キャッシュが効かない問題から不採用。

**Mutation**:
上位層（フレームワーク・ECS・Signal・VDOM reconciler 等）が NewDOM に送る変更操作。`nd_node_create` / `nd_node_update` / `nd_node_destroy` / `nd_node_set_parent` がその手段。NewDOM は mutation を受け取るだけであり、状態変化を自ら検知しない。

**Backend**:
GPU API 抽象層。NewDOM は wgpu を唯一の Backend として使用し、wgpu が Vulkan / Metal / DX12 / WebGPU（ブラウザ）への変換を担う。NewDOM は独自の Backend 抽象を持たない。
_Avoid_: Renderer, Driver

**Binding**:
newdom.h（C ABI）を各言語の FFI 機構でラップしたもの。TypeScript / Python / Swift / Kotlin 等。Binding は薄いラップであり、ロジックを持たない。
_Avoid_: SDK, Wrapper, Port

**DOM Adapter**:
NewDOM コア（C ABI）の上に乗る独立した adapter 層（crate: `newdom-dom`）。設計目標はブラウザの DOM と同等の開発者体験を提供すること。`createElement` / `appendChild` / `getElementById` / `querySelector` / `addEventListener` / `dispatchEvent` 等の DOM 互換 API を提供する。プラットフォームのイベントループを所有し（winit 等）、raw input を受け取って `nd_hit_test` で NodeId を解決し、click / focus / blur の合成・バブリングを担う。`element.style` への代入は `nd_begin_frame()` 直前にバッチで `nd_node_update` へ変換する。`createElement(type)` の型文字列は DOM Adapter 固有の語彙（`"view"`, `"rect"`, `"text"`, `"image"`, `"canvas"` 等）を使い、HTML タグ名（`"div"`, `"span"` 等）は上位の HTML adapter 層が変換する。`createTextNode(text)` はコアの `ND_NODE_TEXT` に 1:1 でマップする。実装は Phase 2 から開始し、初版では API の全域を実装せず段階的に拡張するが、設計の北極星はフル DOM 互換である。Binding（薄いラップ）とは明確に異なる。
_Avoid_: DOM Layer, Web Layer, HTML Adapter（HTML Parser を内包すると誤解される）

**C ABI**:
newdom.h として公開される関数群。Rust の `extern "C"` + cbindgen で生成する。すべての Binding はこの C ABI を通じて NewDOM と通信する。
_Avoid_: API, Interface（文脈が曖昧な場合）

**Dirty Region**:
前フレームから変化のあった描画領域。**Phase 0 では全画面再描画を許容する**。Dirty Region による部分再描画は Phase 1 以降の最適化である。ただし retained object store により text shaping / layout 計算 / glyph atlas は常にキャッシュされる。

**Glyph Atlas**:
レンダリング済みグリフを格納する GPU テクスチャ。LRU でエビクションし、UV 座標でアドレス指定する。

**Layout（Optional Module）**:
NewDOM コアの必須機能ではなく、optional な上位モジュール。Layout Engine は最終的に `nd_node_update(id, { x, y, width, height })` を呼ぶ producer の一種であり、NewDOM コアから見れば通常の Mutation と区別がない。実装は `newdom-layout` crate として `newdom-core` から分離する。

**Phase 0**:
「ブラウザの canvas に wgpu + Vello で色付き矩形が描画される」状態。C ABI・レイアウト・テキストは Phase 0 のスコープ外。完了時に Qiita に投稿する。

## Example Dialogue

> 「NewDOM は React の代替か？」
> → 「違う。React は NewDOM の上に乗る producer の一種。React が VDOM diff を取り、変化分を `nd_node_update()` に流す。NewDOM は受け取って描くだけ」

> 「Node を追加したい」
> → 「`nd_node_create(kind)` で NodeId を受け取り、`nd_node_update(id, props)` で位置・色を設定する。`nd_begin_frame()` / `nd_end_frame()` で GPU に送る」

> 「レイアウトはどこでやる？」
> → 「newdom-layout が Taffy で計算した x/y/width/height を `nd_node_update()` で NewDOM に渡す。NewDOM コアはその値を描くだけで、どう計算されたかを知らない」

> 「全部再描画したい」
> → 「Phase 0 では全 Node を `nd_node_update()` で通知すれば全体が再描画される。Dirty Region による部分再描画は Phase 1 以降」
