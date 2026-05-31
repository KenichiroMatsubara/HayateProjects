Status: ready-for-agent

# PRD: NewDOM — GPU-Native UI Rendering Substrate

## Problem Statement

現代の UI スタックは、アプリケーションロジックと GPU の間に 5〜7 層の変換（Framework → Virtual DOM → HTML DOM → CSS Reflow → Paint → Composite）が挟まる。この構造は IDE・Infinite Canvas・AI チャット・グラフ可視化・ゲーム HUD など「document model と合わないアプリ」のパフォーマンスと表現力を根本的に制約している。

既存の代替（Flutter/Impeller・Xilem/Vello・Makepad・React Native Skia・Unity UI）は特定の言語・エコシステム・ユースケースに強く依存しており、「言語非依存の汎用 GPU 描画 substrate」は存在しない。

UI フレームワーク作者、ネイティブアプリ開発者、Web 開発者の誰もが、言語を問わず、プラットフォームを問わず使える「描画のカーネル」が必要とされている。

## Solution

**NewDOM** は GPU-native retained scene graph、言語非依存 C ABI、最小限のレイアウトエンジン、テキスト/ベクターレンダリングパイプラインの総体である。

フレームワークでも状態管理でもなく、描画の共通語として機能する。TypeScript でも Python でも Swift でも Kotlin でも、どんな言語の UI フレームワークも NewDOM の上で動く。WebGPU でも Vulkan でも Metal でも DX12 でも、wgpu 経由でその上に薄く乗る。

Phase 0 の完了定義：**ブラウザの canvas に wgpu + Vello で色付き矩形が描ける。Qiita に投稿する。**

## User Stories

### Substrate ユーザー（フレームワーク作者）

1. As a UI framework author, I want a retained scene graph API, so that I can send only changed nodes each frame without rebuilding the entire tree.
2. As a UI framework author, I want a language-agnostic C ABI (newdom.h), so that I can bind NewDOM from any language without maintaining a Rust dependency.
3. As a UI framework author, I want NewDOM to handle dirty region tracking, so that my framework does not need to implement partial invalidation logic.
4. As a UI framework author, I want NewDOM to be single-threaded by default, so that I can integrate it without dealing with thread-safety constraints in Phase 0.
5. As a React-like framework author, I want to call nd_node_update() for only the changed nodes each frame, so that reconciliation maps directly to scene graph mutations.
6. As a Signal-based framework author, I want nd_node_update() to accept a partial props diff, so that reactive updates map to minimal GPU work.
7. As an ECS-based framework author, I want to map my entity IDs to NodeIds, so that my component system can drive scene graph updates.

### Web 開発者

8. As a TypeScript developer, I want to import newdom.wasm and draw UI primitives on a canvas, so that I can build GPU-accelerated web UIs without a DOM overhead.
9. As a TypeScript developer, I want nd_node_create / nd_node_update / nd_begin_frame as WASM exports, so that I can call NewDOM from TypeScript without a separate SDK in Phase 0.
10. As a TypeScript developer, I want a higher-level TypeScript SDK (@newdom/web) in a later phase, so that I get IDE autocompletion and ergonomic class-based APIs.
11. As a web developer, I want NewDOM to use the browser's native WebGPU API, so that no additional GPU runtime is loaded.
12. As a web developer, I want NewDOM to target wasm32-unknown-unknown, so that the WASM module is minimal and loads fast.
13. As a SPA developer, I want NewDOM's frame loop to be driven by requestAnimationFrame, so that rendering is synchronized with the browser's refresh cycle.

### ネイティブアプリ開発者

14. As an Android developer, I want NewDOM to run via wgpu on Vulkan, so that I can build GPU-native Android UIs with the same NewDOM API as the web.
15. As an iOS developer, I want NewDOM to run via wgpu on Metal, so that I get native GPU performance on Apple platforms.
16. As a Windows developer, I want NewDOM to run via wgpu on DX12/Vulkan, so that I can ship a high-performance desktop app.
17. As a native app developer, I want newdom.h and a compiled dylib/static lib, so that I can link NewDOM into any native application.
18. As a Python developer, I want ctypes/cffi bindings generated from newdom.h, so that I can build Python UI tools on top of NewDOM.

### OSS コントリビューター

19. As an OSS contributor, I want a Cargo workspace with clearly separated crates, so that I can contribute to a single module without understanding the entire codebase.
20. As an OSS contributor, I want vendored dependencies under crates/vendor/, so that I can understand exactly which version of each dependency NewDOM uses.
21. As an OSS contributor, I want tests for each deep module's external behavior, so that I can refactor internals without breaking the interface contract.
22. As an OSS contributor, I want MIT license, so that I can build commercial products on top of NewDOM without legal friction.

### テキスト・レイアウト

23. As a UI developer, I want ND_NODE_TEXT with parley-shaped text, so that I can render Latin text with correct kerning and ligatures.
24. As a UI developer, I want font fallback via fontique, so that CJK / emoji / Arabic text automatically selects the correct system font.
25. As a UI developer, I want ND_NODE_CONTAINER with FLEX layout via Taffy, so that I can position child nodes using Flexbox semantics.
26. As a UI developer, I want incremental layout recomputation, so that only the dirty subtree is recalculated each frame.
27. As a UI developer, I want ABSOLUTE layout mode, so that I can position nodes at explicit coordinates.

### 描画プリミティブ

28. As a UI developer, I want ND_NODE_RECT with fill color and corner radius, so that I can draw rounded rectangles as the foundational UI primitive.
29. As a UI developer, I want ND_NODE_PATH for vector shapes, so that I can render arbitrary 2D geometry without rasterizing to a texture.
30. As a UI developer, I want ND_NODE_IMAGE for bitmap textures, so that I can display photos and icons.
31. As a UI developer, I want ND_NODE_LAYER with opacity and blend_mode, so that I can compose groups of nodes with GPU layer compositing.
32. As a UI developer, I want ND_NODE_HIT_REGION for event detection, so that I can define interactive areas without visual rendering.
33. As a UI developer, I want nd_hit_test(x, y) returning a NodeId, so that I can map pointer events to scene graph nodes.

### パフォーマンス

34. As a performance-critical app developer, I want dirty region partial redraws, so that unchanged parts of the screen reuse cached GPU textures.
35. As a performance-critical app developer, I want the scene graph to be backed by a slotmap (generational arena), so that Node lookups are O(1) with contiguous memory access.
36. As a performance-critical app developer, I want Vello's GPU compute shader path rendering, so that vector graphics and text render at GPU-native speed.
37. As a performance-critical app developer, I want a glyph atlas with LRU eviction, so that repeated text rendering does not re-rasterize glyphs each frame.

## Implementation Decisions

### Cargo ワークスペース構造

Cargo workspace を採用し、以下の crate 構成で開始する：

- **newdom-core**: Scene Graph・Layout・Text・Render の全コアロジック。wasm と native の両方でリンクされる
- **newdom-wasm**: WASM エントリポイント。wasm-pack でビルドし、C ABI 関数を WASM エクスポートとして公開
- **newdom-ffi**: C ABI 生成（cbindgen）。Phase 1 以降で追加
- **crates/vendor/**: Vello・Taffy・parley・fontique・skrifa のベンダリングコピー

### 依存スタック

| 役割 | crate | 管理方法 |
|---|---|---|
| GPU バックエンド | wgpu | Cargo.toml 依存（upstream 追従） |
| 2D レンダリング | Vello | ベンダリング（upstream から自律） |
| レイアウト | Taffy | ベンダリング |
| テキスト layout | parley | ベンダリング |
| フォント管理 | fontique | ベンダリング |
| フォント解析 | skrifa | ベンダリング |
| NodeId 管理 | slotmap | Cargo.toml 依存 |

wgpu はプラットフォーム対応の追従コストが高すぎるため Cargo.toml 依存として維持する。それ以外の主要依存はベンダリングし、upstream の破壊的変更から NewDOM を守る。upstream の改良は任意のタイミングで cherry-pick する。

### NodeId 設計

NewDOM が slotmap で NodeId を払い出す。上層フレームワークは返された NodeId を保持し、「どの entity が どの NodeId か」のマッピングを自身で管理する。C ABI では `uint64_t` として公開し、slotmap の generational key を bit-cast する。削除済み Node への誤 update は generational check で検出しエラーを返す。

### GPU バックエンド方針

wgpu を唯一の GPU バックエンドとして採用し、NewDOM は独自の Backend 抽象を持たない。wgpu が Vulkan / Metal / DX12 / WebGPU（ブラウザ）へのプラットフォーム変換を担う。WebGPU 仕様で露出されない低レベル GPU 最適化が将来必要になった場合は、NewDOM コアを変更するのではなく外部拡張として接続する。

### スレッドモデル

シングルスレッド。Scene Graph 更新・Layout 計算・Render Command 生成・GPU 送信を単一スレッドで実行する。WASM 環境は SharedArrayBuffer なしでシングルスレッド前提であり、wgpu も `!Send` な型を持つ。マルチスレッド化は API 安定後の将来 ADR として予約する。

### Vello 統合

NewDOM は Vello のレンダリングアルゴリズム（GPU compute shader による 2D path rendering）を自前実装しない。Vello の vendored コピーを使用し、Scene Graph → Vello Scene への変換レイヤー（vello_bridge）を介して呼び出す。この変換レイヤーが Vello の API 変更を吸収する防火壁となる。

### テキストパイプライン

Linebender スタック（parley + fontique + skrifa）を採用する。cosmic-text より組み立てコストが高いが、Vello と同一チーム設計であり将来の統合が自然になる。Phase 0 では Latin テキスト 1 行のみを対象とし、CJK / Bidi は Phase 2 で対応する。

### Web インターフェース

Phase 0〜1 は WASM モジュールが C ABI 関数をそのままエクスポートする（B 方式）。TypeScript 開発者は WASM exports を直接呼ぶ。Phase 1 以降で TypeScript SDK（@newdom/web）を別 package として追加し、ergonomic なクラスベース API を提供する。C ABI が唯一の真実であり、SDK はその薄いラッパーに留まる。

### Phase 0 完了定義

「ブラウザの HTML canvas 要素に、wgpu + Vello を経由して、任意の fill color を持つ矩形が描画される」状態。C ABI・レイアウト・テキストは Phase 0 のスコープ外。完了時に Qiita に投稿する。

## Testing Decisions

### テストの原則

- 外部から観測できる振る舞いのみをテストする。slotmap の内部実装・Vello の描画パイプライン内部・Taffy のソルバーアルゴリズムはテスト対象外
- モジュールの公開インターフェースに対してテストを書く
- GPU を必要とするテスト（Vello 描画の正しさ）は headless wgpu を使う

### テスト対象モジュール

| モジュール | テスト内容 |
|---|---|
| **scene_graph** | Node の CRUD、親子関係の追加/移動/削除、削除済み NodeId への update でのエラー返却、slotmap の generational check |
| **layout** | Flex コンテナ内の子 Node の位置計算結果、dirty flag の伝播（変更した Node の親が dirty になること）、clean な Node が再計算されないこと |
| **vello_bridge** | NdNodeProps から Vello の描画オブジェクトへの変換が正しいパラメータを持つこと（color・size・corner radius） |
| **text_pipeline** | Latin 文字列が shaped glyphs に変換されること、フォントが fontique で解決されること |

### 先行実装のない新規プロジェクトのため

crate ごとの unit test を `#[cfg(test)]` モジュールで書く。Vello・wgpu を使う統合テストは `tests/` ディレクトリに分離し、headless GPU 環境でのみ実行する。

## Out of Scope

このPRD のスコープ外（Phase 1 以降）：

- C ABI（newdom-ffi crate・cbindgen）
- Taffy レイアウト統合
- CJK / Bidi / IME テキスト
- 画像・ベクターパス（ND_NODE_IMAGE / ND_NODE_PATH）
- Hit testing（nd_hit_test）
- Layer compositing（opacity / blend_mode / shadow）
- Android / iOS ネイティブバイナリ
- TypeScript SDK（@newdom/web）
- Python / Swift / Kotlin バインディング
- アニメーション primitive
- アクセシビリティ tree
- DevTools / プロファイラ
- WebGL2 fallback

## Further Notes

- OSS ライセンスは MIT。商用利用・プロプライエタリフレームワークによる上乗せを制限しない。Vello 等 Apache-2.0 依存のライセンス表示は `LICENSES/` ディレクトリで管理し、`cargo-about` で自動生成する
- Phase 0 完了後は Qiita に連載形式で投稿し、各 Phase の完了ごとに記事を追加する。「動くものを公開し、実況しながら育てる」がこのプロジェクトの OSS 戦略
- Vello・Taffy・parley 等の vendored 依存は upstream を無視するのではなく、NewDOM の都合で任意のタイミングに upstream の改良を cherry-pick する方針
- 参照した仕様書: newdom-spec.md v0.1（2026）
