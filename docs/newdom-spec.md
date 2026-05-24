# NewDOM — 設計仕様書 v0.3

> **v0.2 との差分**: アーキテクチャ思想を「アプリへの同封」から「Platform Adapter による描画基盤」へ格上げ。Browser Extension・Tauri・Native Runtime としての動作を設計に組み込む。HTML/CSS 互換を非目標から削除し、CSS エンジン同封（Taffy→Servo→Blink 互換）による段階的実現を方針として確定（ADR-0011）。各変更の経緯は `docs/adr/` を参照。v0.2 原文は `docs/archive/` に移動予定。

---

## 0. 哲学

> **"描くことを知らず、描く"**

NewDOM はフレームワークではない。  
NewDOM は言語でもない。  
NewDOM は **描画の共通語** である。

上に何が乗ってもいい。TypeScript でも Python でも Swift でも Kotlin でも、どんな言語の UI フレームワークも NewDOM の上で動く。  
下に何があってもいい。WebGPU でも Vulkan でも Metal でも DX12 でも、wgpu がその差異を吸収する。

**NewDOM はアプリへの同封を前提としない。**  
Web アプリに組み込まれるだけでなく、Browser Extension として既存ページに注入され、Tauri Runtime として動き、将来のブラウザの描画基盤になりうる。Platform Adapter がプラットフォームとの仲介を担い、NewDOM Core は描画のみに集中する。

---

## 1. 問題定義

### 1.1 現在の Web UI スタックの構造的問題

```
Application Code
      ↓
Framework (React / Vue / Svelte ...)
      ↓
Virtual DOM / Reconciler
      ↓
HTML DOM
      ↓
Style / Layout (CSS Reflow)
      ↓
Paint
      ↓
Composite (GPU)
```

GPU はスタックの最下層にしかない。アプリケーションロジックから GPU まで 5〜7 層の変換が挟まる。

### 1.2 現代アプリが document model と合わない

| アプリ種別 | DOM との相性 |
|---|---|
| IDE / Code Editor | ❌ 大量の行、ミニマップ、オーバーレイ |
| Infinite Canvas | ❌ document に座標系がない |
| AI チャット UI | ⚠️ ストリーミングでの DOM 操作コスト |
| Realtime Collab | ❌ 頻繁な Mutation、CRDT との統合が困難 |
| Graph Visualization | ❌ SVG は遅い、Canvas は状態管理を持たない |
| Game UI / HUD | ❌ reflow が致命的 |

### 1.3 既存の代替の問題

| 技術 | 問題 |
|---|---|
| Flutter/Impeller | Dart 必須、widget tree に強く依存 |
| Xilem/Vello | Rust エコシステムに閉じている、Web バインディングが弱い |
| Makepad | IDE 向けに特化、汎用 Substrate ではない |
| React Native Skia | React に強依存 |
| Unity/Godot UI | ゲームエンジン前提、Web では重すぎる |

**誰も「言語非依存・プラットフォーム非依存の汎用 GPU 描画 Substrate」を作っていない。**

---

## 2. NewDOM の定義

### 2.1 What NewDOM IS

```
NewDOM Core = GPU-Native Retained Scene Graph
            + Text/Vector Rendering Pipeline (Vello)
            + wgpu Backend

NewDOM Platform Adapter = CSS Engine + Layout Engine + Event/IME 取得
                        + NewDOM Core への Mutation 変換
                        （プラットフォームごとに異なる実装）
```

**NewDOM Core は描画の "カーネル" である。**  
OS のカーネルが言語を選ばず全てのプログラムの下にあるように、NewDOM Core は UI フレームワークを選ばず全ての描画の下にある。

**NewDOM Platform Adapter は Core とプラットフォームを仲介する。**  
Browser Extension・Tauri・Native・将来のブラウザ等、プラットフォームごとに異なる Adapter が CSS Engine の出力（Absolute Layout Tree）を NewDOM Mutation に変換する。Core は Adapter を知らない。

### 2.2 What NewDOM IS NOT

- ❌ フレームワーク（React / Vue の代替ではない）
- ❌ 状態管理（Redux / Signal の代替ではない）
- ❌ CSS エンジン（Core の責務ではない。Platform Adapter 層が持つ）
- ❌ 特定言語のライブラリ
- ❌ 特定アプリへの同封を前提とした SDK

### 2.3 ポジショニング

```
┌──────────────────────────────────────────────────────┐
│   Web App   │  Browser Extension  │  Tauri  │ Native │  ← 上層（何でもよい）
├──────────────────────────────────────────────────────┤
│        HTML/CSS/JS  ←→  Platform Adapter             │  ← CSS Engine + Layout
│   (Taffy → Servo → Blink 互換)                       │     + Event/IME 取得
├──────────────────────────────────────────────────────┤
│                                                       │
│              N E W D O M   C O R E                   │  ← ここ
│    (C ABI / WASM / FFI で接続)                       │
│                                                       │
├──────────────────────────────────────────────────────┤
│        wgpu                                           │  ← GPU 抽象（固定）
├──────────────────────────────────────────────────────┤
│  WebGPU   Vulkan   Metal   DX12                      │  ← GPU バックエンド
└──────────────────────────────────────────────────────┘
```

### 2.4 HTML/CSS 互換に対する立場

NewDOM Core は CSS エンジンを持たない。  
しかし Platform Adapter 層が CSS エンジン（ADR-0011）を保持し、Absolute Layout Tree を NewDOM Mutation に変換することで、HTML/CSS ページの GPU 描画を段階的に実現する。

CSS エンジンの段階的改善：

```
Phase 2: Taffy（Flexbox/Grid。CSS cascade なし）
Phase 3: Servo/stylo（CSS cascade + フルレイアウト）
Phase 5: Blink 互換（完全 HTML/CSS 互換）
```

`getComputedStyle()` + `getBoundingClientRect()` によるブラウザ計算結果の抽出は採用しない。ブラウザの reflow コストが消えず、ペイントを置き換えるだけでは性能改善にならないため（ADR-0010, ADR-0011）。

---

## 3. アーキテクチャ

### 3.1 実装言語

**Rust**（ADR-0001）。

wgpu は Rust ネイティブのライブラリであり、Rust で統一することで cargo 一本に収まり、クロスコンパイル・WASM（wasm-pack）・C ABI 生成（cbindgen）が一貫したツールチェーンで完結する。

### 3.2 全体構成

```
┌──────────────────────────────────────────────────────────────┐
│                      PLATFORM ADAPTERS                        │
│                                                               │
│  ┌─────────────────┐  ┌──────────────┐  ┌─────────────────┐  │
│  │ Browser Extension│  │    Tauri     │  │  DOM Adapter    │  │
│  │  (newdom-ext)   │  │(newdom-tauri)│  │  (newdom-dom)   │  │
│  │                 │  │              │  │                 │  │
│  │  content script │  │   Window     │  │ createElement   │  │
│  │  CSS Engine     │  │  CSS Engine  │  │ addEventListener │  │
│  │  HTML Parser    │  │  HTML Parser │  │                 │  │
│  └────────┬────────┘  └──────┬───────┘  └───────┬─────────┘  │
│           └──────────────────┴──────────────────┘            │
│                    Absolute Layout Tree                       │
│                    → NewDOM Mutation                          │
└──────────────────────────────┬───────────────────────────────┘
                               │ C ABI (newdom.h) / WASM
┌──────────────────────────────▼───────────────────────────────┐
│                  LANGUAGE BINDINGS                            │
│  TypeScript  │  Python  │  Swift  │  Kotlin  │  C++          │
└──────────────────────────────┬───────────────────────────────┘
                               │
┌──────────────────────────────▼───────────────────────────────┐
│                  NEWDOM CORE (Rust)                           │
│                                                               │
│  ┌────────────────┐  ┌─────────────────┐                     │
│  │  Scene Graph   │  │  newdom-layout  │ (optional)          │
│  │  (newdom-core) │  │  Taffy / Flex   │                     │
│  └───────┬────────┘  └────────┬────────┘                     │
│          └──────────────┬─────┘                              │
│                         │  Mutation                          │
│          ┌──────────────▼──────────────┐                     │
│          │  SceneGraph → Vello Scene   │                     │
│          │  (薄い変換レイヤー)          │                     │
│          └──────────────┬──────────────┘                     │
└─────────────────────────┼────────────────────────────────────┘
                          │
┌─────────────────────────▼────────────────────────────────────┐
│                    Vello  (vendored)                          │
│     GPU compute 2D renderer (wgpu ベース)                    │
└─────────────────────────┬────────────────────────────────────┘
                          │
┌─────────────────────────▼────────────────────────────────────┐
│                    wgpu                                       │
│   WebGPU (WASM)  │  Vulkan  │  Metal  │  DX12               │
└──────────────────────────────────────────────────────────────┘
```

### 3.3 crate 構成

| crate | 役割 | フェーズ |
|---|---|---|
| `newdom-core` | Scene Graph（NodeId, NodeKind, SceneGraph）。wasm-bindgen 依存なし | 現在 |
| `newdom-wasm` | WASM バインディング。wgpu + Vello。`NdRenderer` が SceneGraph と GPU 状態を所有 | 現在 |
| `newdom-layout` | Taffy による Layout Engine（optional）。CSS cascade なし | Phase 1〜 |
| `newdom-dom` | DOM Adapter（`createElement` / `addEventListener` 等）| Phase 2〜 |
| `newdom-extension` | Browser Extension Platform Adapter。HTML Parser + CSS Engine + content script | Phase 4〜 |
| `newdom-tauri` | Tauri Platform Adapter。Tauri Window + CSS Engine | Phase 4〜 |

### 3.4 Platform Adapter の責務

Platform Adapter は Core を知っているが、**Core は Adapter を知らない**。

Adapter が担う責務：
- HTML 取得・パース
- CSS 取得・カスケード計算
- レイアウト計算（Absolute Layout Tree 生成）
- Event 取得・正規化
- IME 取得
- Absolute Layout Tree → NewDOM Mutation 変換

Core が担う責務：
- Mutation の受け取りと Scene Graph 更新
- Scene Graph → Vello Scene 変換
- GPU への描画命令送出

### 3.5 スレッドモデル

シングルスレッド（ADR-0003）。wgpu の `!Send` 型と WASM 環境の制約により、Phase 0 はシングルスレッドで設計する。`NdRenderer` は `!Send + !Sync`。レンダースレッド分離は API 安定後の将来 ADR として予約。

---

## 4. Scene Graph 仕様

### 4.1 Node 型

NewDOM の Scene Graph は GPU が直接理解できる型のみで構成される。

```rust
pub enum NodeKind {
    // Phase 0
    Rect { x: f32, y: f32, width: f32, height: f32, color: [f32; 4] },

    // Phase 1〜
    // Text   { text_id: TextId, ... }
    // Image  { image_id: ImageId, fit: ImageFit }
    // Path   { ... }
    // Container { ... }
    // Layer  { opacity: f32, blend_mode: BlendMode }
    // HitRegion { ... }
}
```

### 4.2 NodeId

`slotmap::DefaultKey`（generational arena）。削除済み Node への誤 update は generational check で検出され、UB ではなく安全に無視される。C ABI では `uint64_t` として公開する（Phase 1〜）。

### 4.3 Retained Graph の更新モデル

```
初回: nd_node_create(kind) -> NodeId
更新: nd_node_update(id, changed_props)  // 変更分のみ
削除: nd_node_destroy(id)
移動: nd_node_set_parent(id, parent_id)
```

NewDOM は Mutation を受け取るだけで、状態変化を自ら検知しない。

---

## 5. 描画パイプライン

### 5.1 フレームループ

```
1. nd_begin_frame()
   └── Mutation 受付開始

2. nd_node_update() × N
   └── Scene Graph を更新

3. nd_end_frame()
   └── SceneGraph → Vello Scene 変換（薄い変換レイヤー）
   └── Vello: GPU compute shader で path rendering
   └── 中間テクスチャ (Rgba8Unorm) → surface blit
   └── surface.present()
```

### 5.2 SceneGraph → Vello Scene 変換レイヤー

Vello の API 変更を NewDOM コアから隔離するための薄いレイヤー（ADR-0006）。

```rust
fn build_vello_scene(graph: &SceneGraph) -> vello::Scene { ... }
```

Scene Graph を深さ優先で走査し、NodeKind に対応する Vello プリミティブへ変換する。この関数のみが Vello の型に触れる。

### 5.3 Dirty Region

Phase 0 では全画面再描画を許容する。Dirty Region による部分再描画は Phase 1 以降の最適化。ただし retained Scene Graph により text shaping / layout 計算 / glyph atlas は常にキャッシュされる。

---

## 6. GPU Backend

wgpu を唯一の Backend として使用する（ADR-0002）。独自の `NdBackend` trait は持たない。

| 環境 | wgpu backend |
|---|---|
| Web (WASM) | ブラウザ WebGPU |
| Android | Vulkan |
| iOS / macOS | Metal |
| Windows | DX12 / Vulkan |
| Linux | Vulkan |

---

## 7. テキストエンジン

Linebender スタック（ADR-0005）。Vello と同チーム設計で自然に統合される。

| crate | 役割 |
|---|---|
| `parley` | text layout（行折り返し、alignment、paragraph） |
| `fontique` | font management（fallback chain、font enumeration） |
| `skrifa` | font parsing（OpenType）|

テキストレンダリングは Phase 1 以降。Phase 0 のスコープ外。

---

## 8. Layout Engine と CSS Engine

### 8.1 Layout Engine（newdom-layout）

`newdom-layout` crate として `newdom-core` から分離する（ADR-0008）。

Layout Engine は最終的に `nd_node_update(id, { x, y, width, height })` を呼ぶ producer の一種であり、NewDOM Core から見れば通常の Mutation と区別がない。

- **Taffy**（Pure Rust）を採用（ADR-0004）。Flexbox + CSS Grid + Block layout
- `newdom-core` は Taffy を import しない
- Layout を使わないユーザー（ゲーム HUD・Infinite Canvas 等）は `newdom-layout` を依存に含めなくてよい

### 8.2 CSS Engine の段階的改善（ADR-0011）

Taffy は MVP 向けの Layout Engine であり、CSS エンジンとしては **CSS cascade を持たない**。HTML/CSS 互換に向けた段階的な改善の第一歩として位置づける。

| フェーズ | CSS Engine | 互換範囲 |
|---|---|---|
| Phase 2 | Taffy | Flexbox/Grid レイアウトのみ。CSS cascade なし |
| Phase 3 | Servo/stylo | CSS cascade + フルレイアウト |
| Phase 5 | Blink 互換 | 完全 HTML/CSS 互換 |

CSS Engine は Platform Adapter 層（`newdom-extension` / `newdom-tauri` 等）に属し、NewDOM Core には入らない。

---

## 9. C ABI / バインディング戦略（Phase 1〜）

Phase 0 では WASM + wasm-bindgen が唯一の公開 API。C ABI は Phase 1 以降。

### 9.1 公開 API（newdom.h、Phase 1〜）

```c
NdCtx*   nd_create(NdConfig* config);
void     nd_destroy(NdCtx* ctx);

NdNodeId nd_node_create(NdCtx* ctx, NdNodeKind kind);
void     nd_node_destroy(NdCtx* ctx, NdNodeId id);
void     nd_node_set_parent(NdCtx* ctx, NdNodeId id, NdNodeId parent);
void     nd_node_update(NdCtx* ctx, NdNodeId id, NdNodeProps* props);

void     nd_begin_frame(NdCtx* ctx);
void     nd_end_frame(NdCtx* ctx);

NdNodeId nd_hit_test(NdCtx* ctx, float x, float y);
```

### 9.2 WASM API（Phase 0 実装済み・拡張中）

```typescript
const renderer = await NdRenderer.init(canvas);

const rectId = renderer.nd_node_create(10, 10, 200, 100, 0.2, 0.5, 1.0, 1.0, 0);
renderer.nd_render(0.1, 0.1, 0.1);
```

---

## 10. DOM Adapter（Phase 2〜）

`newdom-dom` crate として NewDOM Core の上に乗る独立した adapter 層。

- `createElement` / `appendChild` / `getElementById` / `querySelector` / `addEventListener` / `dispatchEvent` 等の DOM 互換 API
- 型文字列は HTML タグ名（`"div"`, `"span"` 等）をそのまま使う（ADR-0009）
- フォーム系要素（`input`, `button`, `select`, `textarea`, `form`）は初版では未サポート
- Binding（薄いラップ）とは明確に異なる

---

## 11. 依存管理

主要依存は workspace 内にベンダリングし upstream から自律する（ADR-0007）。

| crate | ベンダー場所 | 理由 |
|---|---|---|
| vello / vello_encoding / vello_shaders | `crates/vendor/vello` 等 | 描画パイプラインの核心 |
| taffy | `crates/vendor/taffy` | レイアウト計算の核心 |
| parley / fontique / skrifa | `crates/vendor/parley` 等 | テキストスタックの核心 |

wgpu は対象外。巨大すぎ、プラットフォーム対応の追従コストが高い。

---

## 12. ロードマップ

### Phase 0 — Prototype（現在）

目標：**ブラウザの canvas に wgpu + Vello で NodeId 経由の色付き矩形が描画される**

- [x] Rust workspace 構成（newdom-core / newdom-wasm）
- [x] wgpu + Vello 初期化（WASM）
- [x] nd_clear（単色塗りつぶし）
- [ ] SceneGraph → Vello Scene 変換レイヤー
- [ ] wasm-bindgen 公開 API（`NdRenderer`、Node CRUD、begin/end frame）
- [ ] ブラウザデモ（矩形を Node 経由で描画）

### Phase 1 — Usable

目標：実際の UI が作れる

- [ ] テキスト描画（Parley + Vello glyph rendering）
- [ ] 画像描画
- [ ] Hit testing（nd_hit_test）
- [ ] Flex レイアウト（newdom-layout / Taffy）
- [ ] C ABI（newdom.h / cbindgen）
- [ ] Dirty Region 部分再描画
- [ ] Vulkan / Metal ネイティブビルド

### Phase 2 — Ecosystem

目標：フレームワークが NewDOM の上に乗れる

- [ ] DOM Adapter 初版（newdom-dom crate）
- [ ] CJK / Bidi テキスト・IME
- [ ] Layer compositing（opacity, blend mode, shadow）
- [ ] Animation primitive（tween, spring）
- [ ] Accessibility tree export（AX API / AT-SPI）
- [ ] Python / Swift / Kotlin Bindings

### Phase 3 — HTML/CSS Engine

目標：既存 Web コンテンツを NewDOM で描画できる基盤を作る

- [ ] Servo/stylo 統合（CSS cascade + フルレイアウト）
- [ ] Absolute Layout Tree → NewDOM Mutation パイプライン
- [ ] Browser Runtime Adapter 初版

### Phase 4 — Extension Runtime

目標：Browser Extension として既存ページを NewDOM で高速描画する

- [ ] newdom-extension crate（HTML Parser + CSS Engine + content script）
- [ ] 既存ページの Absolute Layout Tree 抽出 → NewDOM 描画
- [ ] snapshot + display:none + nd_hit_test + dispatchEvent によるイベント処理
- [ ] 既存 Web サイト高速描画実験

### Phase 5 — NewDOM Browser

目標：完全 HTML/CSS 互換の GPU 描画基盤

- [ ] Blink 互換 CSS Engine
- [ ] 完全 HTML/CSS 互換
- [ ] Tauri Runtime（newdom-tauri crate）
- [ ] Native Runtime

---

## 13. 非目標（やらないこと）

| 項目 | 理由 |
|---|---|
| CSS エンジンを Core に実装すること | Platform Adapter 層の責務。Core は描画のみ |
| ブラウザの CSS 計算結果（getComputedStyle）への依存 | reflow コストが消えない（ADR-0010, ADR-0011） |
| 状態管理 | 上層の仕事 |
| コンポーネントシステム | 上層の仕事 |
| ルーティング | 上層の仕事 |
| アクセシビリティの完全対応 | Phase 2 以降。初期は AX tree の export API のみ |
| サーバーサイドレンダリング | SVG/PDF エクスポートとして別途検討 |
| 独自 GPU Backend 抽象 | wgpu がその役割を担う（ADR-0002） |
| 独自 GPU compute path renderer | Vello がその役割を担う（ADR-0006） |

---

## Appendix A — Node Props（Phase 0 実装範囲）

```rust
pub enum NodeKind {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],  // RGBA, 0.0–1.0
        corner_radius: f32,
    },
}
```

---

## Appendix B — エラー設計（Phase 1〜 C ABI 向け）

```c
typedef enum NdError {
    ND_OK              = 0,
    ND_ERR_INVALID_ID  = 1,
    ND_ERR_GPU         = 2,
    ND_ERR_OOM         = 3,
    ND_ERR_TEXT        = 4,
    ND_ERR_BACKEND     = 5,
} NdError;
```

---

*NewDOM Specification v0.3 — 2026*  
*"Document web は HTML が作った。Application web は NewDOM が作る。Browser の次は NewDOM が描く。"*
