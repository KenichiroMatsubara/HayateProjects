# NewDOM — GPU-Native UI Rendering Substrate
## 設計仕様書 v0.1

---

## 0. 哲学

> **"描くことを知らず、描く"**

NewDOM はフレームワークではない。  
NewDOM は言語でもない。  
NewDOM は **描画の共通語** である。

上に何が乗ってもいい。TypeScript でも Python でも Swift でも Kotlin でも、どんな言語の UI フレームワークも NewDOM の上で動く。  
下に何があってもいい。WebGPU でも Vulkan でも Metal でも DX12 でも、NewDOM はその上に薄く乗る。

HTML/DOM は "document web" を作った。  
NewDOM は "application layer" の描画インフラを作る。

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

GPU はスタックの最下層にしかない。  
アプリケーションロジックから GPU まで、5〜7 層の変換が挟まる。

### 1.2 現代アプリが document model と合わない

| アプリ種別 | DOM との相性 |
|---|---|
| IDE / Code Editor | ❌ 大量の行、ミニマップ、オーバーレイ |
| Infinite Canvas | ❌ document に座標系がない |
| AI チャット UI | ⚠️ ストリーミングでの DOM 操作コスト |
| Realtime Collab | ❌ 頻繁な mutation、CRDT との統合が困難 |
| Graph Visualization | ❌ SVG は遅い、Canvas は状態管理を持たない |
| Game UI / HUD | ❌ reflow が致命的 |

### 1.3 既存の代替の問題

| 技術 | 問題 |
|---|---|
| Flutter/Impeller | Dart 必須、widget tree に強く依存 |
| Xilem/Vello | Rust エコシステムに閉じている、Web バインディングが弱い |
| Makepad | IDE 向けに特化、汎用 substrate ではない |
| React Native Skia | React に強依存 |
| Unity/Godot UI | ゲームエンジン前提、Web では重すぎる |

**誰も「言語非依存の汎用 GPU 描画 substrate」を作っていない。**

---

## 2. NewDOM の定義

### 2.1 What NewDOM IS

```
NewDOM = GPU-Native Retained Scene Graph
       + Language-Agnostic C ABI
       + Minimal Layout Engine
       + Text/Vector Rendering Pipeline
       + Platform Backend Abstraction
```

**NewDOM は描画の "カーネル" である。**  
OS のカーネルが言語を選ばず、全てのプログラムの下にあるように、  
NewDOM は UI フレームワークを選ばず、全ての描画の下にある。

### 2.2 What NewDOM IS NOT

- ❌ フレームワーク（React / Vue の代替ではない）
- ❌ 状態管理（Redux / Signal の代替ではない）
- ❌ HTML の代替
- ❌ ブラウザの代替
- ❌ 特定言語のライブラリ

### 2.3 ポジショニング

```
┌─────────────────────────────────────────┐
│  TypeScript  Python  Swift  Kotlin  Rust │  ← 上層言語（何でもよい）
├─────────────────────────────────────────┤
│   React   Svelte   SolidJS   custom     │  ← UI フレームワーク（何でもよい）
├─────────────────────────────────────────┤
│                                         │
│              N E W D O M               │  ← ここ
│      (C ABI / WASM / FFI で接続)        │
│                                         │
├─────────────────────────────────────────┤
│  WebGPU   Vulkan   Metal   DX12   GL   │  ← GPU バックエンド（何でもよい）
└─────────────────────────────────────────┘
```

---

## 3. アーキテクチャ

### 3.1 全体構成

```
┌──────────────────────────────────────────────────────┐
│                    LANGUAGE BINDINGS                  │
│  TypeScript  │  Python  │  Swift  │  Kotlin  │  C++  │
└──────────────────────┬───────────────────────────────┘
                       │  C ABI (newdom.h)
┌──────────────────────▼───────────────────────────────┐
│                     CORE (C / Zig)                    │
│                                                       │
│  ┌────────────┐  ┌───────────────┐  ┌─────────────┐ │
│  │ Scene Graph│  │ Layout Engine │  │   Text      │ │
│  │  (Retained)│  │(Flex/Grid/    │  │  Engine     │ │
│  │            │  │ Constraint)   │  │(shaping/IME)│ │
│  └─────┬──────┘  └───────┬───────┘  └──────┬──────┘ │
│        └─────────────────┴──────────────────┘        │
│                          │                            │
│              ┌───────────▼──────────┐                │
│              │   Render Command     │                │
│              │   Encoder            │                │
│              └───────────┬──────────┘                │
└──────────────────────────┼───────────────────────────┘
                           │
┌──────────────────────────▼───────────────────────────┐
│                  BACKEND ABSTRACTION                  │
│   WebGPU  │  Vulkan  │  Metal  │  DX12  │  OpenGL   │
└──────────────────────────────────────────────────────┘
```

### 3.2 コアの実装言語

**C または Zig** で実装する。

理由：
- C ABI を自然に持つ
- FFI コストが最小
- WASM にコンパイル可能
- あらゆる言語からバインディングを生成できる
- Rust より学習コストが低く、コントリビュータを集めやすい

> 参考：SQLite は C で書かれているから全言語から使える。NewDOM も同じ戦略を取る。

---

## 4. Scene Graph 仕様

### 4.1 Node 型

NewDOM の Scene Graph は以下の Node 型のみで構成される。  
DOM の `div/span/p` ではなく、GPU が直接理解できる型。

```c
typedef enum NdNodeKind {
    ND_NODE_CONTAINER,   // 子を持つコンテナ（transform, clip 付き）
    ND_NODE_RECT,        // 矩形（角丸、グラデーション、影）
    ND_NODE_TEXT,        // テキスト（shaping 済み）
    ND_NODE_IMAGE,       // ビットマップ / テクスチャ
    ND_NODE_PATH,        // ベクターパス（SVG 相当）
    ND_NODE_CANVAS,      // 即時描画領域（escape hatch）
    ND_NODE_SURFACE,     // 別 GPU テクスチャへの描画ターゲット
    ND_NODE_LAYER,       // GPU レイヤー（合成、opacity、blend mode）
    ND_NODE_HIT_REGION,  // イベント検出領域（描画なし）
} NdNodeKind;
```

### 4.2 Node の基本構造

```c
typedef struct NdNode {
    uint64_t    id;           // 一意 ID（上層が管理）
    NdNodeKind  kind;
    NdTransform transform;    // 2D affine transform
    NdRect      clip;         // クリッピング矩形
    float       opacity;
    NdBlendMode blend_mode;
    bool        visible;
    uint32_t    child_count;
    uint64_t*   children;     // 子 Node ID の配列
} NdNode;
```

### 4.3 Retained Graph の更新モデル

```
初回: nd_node_create(id, kind, props)
更新: nd_node_update(id, changed_props)   // 変更分のみ
削除: nd_node_destroy(id)
移動: nd_node_reparent(id, new_parent_id)
```

**NewDOM は差分のみ受け取る。** フレームワーク側が何を変えたかを知らせる。  
NewDOM は変更を GPU Scene Graph に反映し、影響を受けた部分のみ再描画する。

---

## 5. Layout Engine

### 5.1 対応レイアウトモード

CSS 全互換は **目指さない**。代わりに実用的な 5 モードを GPU-native で再設計する。

| モード | 概要 | 参考 |
|---|---|---|
| `FLEX` | Flexbox 相当（主軸/交差軸） | CSS Flexbox |
| `GRID` | 固定・可変グリッド | CSS Grid のサブセット |
| `CONSTRAINT` | 相対位置拘束（anchor ベース） | SwiftUI / Figma |
| `ABSOLUTE` | 絶対座標 | CSS position:absolute |
| `CANVAS` | 座標系なし（infinity scroll 等） | ゲームエンジン方式 |

### 5.2 Layout の分離原則

Layout は **CPU で計算し、結果を GPU に送る**。  
GPU で layout 計算は行わない（依存グラフが複雑になるため）。

```
Layout Inputs (CPU)
  → Constraint Solver
  → Computed Rects
  → Scene Graph へ transform として書き込む
  → GPU は transform を受け取るだけ
```

### 5.3 Incremental Layout

変更されたノードから上位に向かって dirty flag を伝播。  
再計算は dirty なサブツリーのみ。

```
node_A (dirty) → parent_B (dirty) → root (dirty)
node_C (clean) → (計算スキップ)
```

---

## 6. Rendering Pipeline

### 6.1 フレームループ

```
1. Invalidation Collection
   └── 上層から届いた node update を集積

2. Layout Pass (CPU)
   └── dirty node のみ再計算

3. Scene Graph Traversal (CPU)
   └── visible node を前から後ろへ走査
   └── Render Command を生成

4. GPU Upload
   └── 変更のあった geometry / texture のみ転送

5. GPU Render Pass
   └── Retained Draw Call を実行
   └── 変更のある Layer のみ再描画

6. Composite
   └── Layer を合成して最終フレームを出力
```

### 6.2 Render Command

GPU に送る命令は全て Command として直列化する。  
これにより backend を差し替えられる。

```c
typedef enum NdCmd {
    ND_CMD_DRAW_RECT,
    ND_CMD_DRAW_PATH,
    ND_CMD_DRAW_TEXT,
    ND_CMD_DRAW_IMAGE,
    ND_CMD_PUSH_LAYER,
    ND_CMD_POP_LAYER,
    ND_CMD_PUSH_CLIP,
    ND_CMD_POP_CLIP,
    ND_CMD_SET_TRANSFORM,
} NdCmd;
```

### 6.3 Partial Invalidation（差分描画）

全画面再描画は行わない。変更した Node に対応する **Dirty Region** のみ再描画。

```
Frame N:   [=====AAAAA=====BBBBB=====]
Frame N+1: [=====AAAAA=====BBBBB=====]
                  ↑ ここだけ変化
→ Dirty Region: A のみ再描画
→ B は GPU Texture をそのまま再利用
```

---

## 7. Text Engine

### 7.1 設計方針

テキストレンダリングは NewDOM 最大の難関。  
**Harfbuzz** (shaping) + **FreeType / CoreText / DirectWrite** (rasterize) + **Vello 方式の GPU atlas** を採用。

### 7.2 パイプライン

```
Input: UTF-8 string + Font + Size + Lang

  ↓ Unicode Segmentation（書記素クラスタ分割）
  ↓ Bidi Algorithm（RTL/LTR 混在対応）
  ↓ Harfbuzz Shaping（ligature、kerning、GPOS/GSUB）
  ↓ Font Fallback（絵文字、CJK、記号の自動フォント切替）
  ↓ Glyph Rasterization（GPU テクスチャアトラスにキャッシュ）
  ↓ Layout（行折り返し、alignment）
  ↓ GPU Draw Call（アトラスから UV 座標で描画）
```

### 7.3 対応機能

| 機能 | 実装方法 |
|---|---|
| Latin / CJK / Arabic | Unicode + Bidi Algorithm |
| 絵文字 | CBDT/SBIX/COLR フォント対応 |
| Ligature / Kerning | Harfbuzz |
| IME（入力途中） | Underline overlay node |
| Selection | Glyph 単位の hit-test |
| Font Fallback | Fontique 相当の fallback chain |
| Variable Font | OpenType fvar / gvar |

### 7.4 Glyph Atlas

```
GPU テクスチャアトラス（2048x2048 等）
┌────────────────────────────────┐
│ aaaa bbbb cccc dddd eeee ffff  │
│ gggg hhhh iiii jjjj kkkk llll  │
│ [emoji] [CJK] [Arabic] ...     │
└────────────────────────────────┘
→ Glyph は UV 座標でアドレス指定
→ LRU でエビクション
→ サイズ変更時はサブピクセルレンダリングの再キャッシュ
```

---

## 8. Reactivity Interface

### 8.1 NewDOM 自身は状態を持たない

Signal でも Observable でも ECS でも Redux でも、何でも接続できる。  
NewDOM が提供するのは **invalidation API** のみ。

```c
// 上層フレームワークが呼ぶ
nd_begin_frame(ctx);
  nd_node_update(ctx, node_id, &props);  // 変化した node を通知
  nd_node_update(ctx, node_id2, &props2);
nd_end_frame(ctx);                       // ここで描画が走る
```

### 8.2 フレームワーク側の責務

```
Framework（React / Solid / 自作）
   ↓
State 変化を検知（Signal / VDOM diff / ECS query）
   ↓
変化した Node の新しい props を計算
   ↓
nd_node_update() を呼ぶ
   ↓
NewDOM が差分描画
```

### 8.3 接続パターン例

**Signal ベース（Solid.js 的）:**
```typescript
// TS バインディング例
const x = signal(100);

effect(() => {
  nd.updateNode(rectId, { x: x.value, width: 200 });
});
```

**VDOM ベース（React 的）:**
```typescript
// Reconciler が diff を取り、変化分を nd.updateNode に流す
reconcile(prevTree, nextTree, (id, patch) => nd.updateNode(id, patch));
```

**ECS ベース（ゲームエンジン的）:**
```rust
// ECS が transform component の変化を検知して通知
for (entity, transform) in changed_transforms.iter() {
    nd_node_update(ctx, entity.id, &transform.into());
}
```

---

## 9. Platform Backend

### 9.1 Backend 抽象層

```c
typedef struct NdBackend {
    void* (*create_buffer)(size_t size, NdBufferUsage usage);
    void  (*upload_buffer)(void* buf, void* data, size_t size);
    void* (*create_texture)(uint32_t w, uint32_t h, NdPixelFormat fmt);
    void  (*begin_render_pass)(NdRenderPassDesc* desc);
    void  (*draw)(NdDrawCall* call);
    void  (*end_render_pass)(void);
    void  (*present)(void);
} NdBackend;
```

上の構造体を実装すれば、どんな GPU API も NewDOM に接続できる。

### 9.2 対応バックエンド

| バックエンド | プラットフォーム | 優先度 |
|---|---|---|
| WebGPU (via Dawn) | Web, Native | ★★★ 最優先 |
| Vulkan | Linux, Android, Windows | ★★★ |
| Metal | macOS, iOS | ★★★ |
| DX12 | Windows | ★★ |
| OpenGL 3.3+ | Fallback | ★ |
| Software (CPU) | テスト用 | ★ |

### 9.3 WebGPU の扱い

Web では **WASM + WebGPU** がメインターゲット。

```
Rust/C で書かれた NewDOM core
  ↓ wasm-pack / emscripten でコンパイル
WASM モジュール (newdom.wasm)
  ↓ JavaScript から import
TypeScript / JavaScript で newdom を操作
  ↓ WebGPU バックエンドが描画
```

ブラウザが WebGPU 非対応の場合は WebGL2 fallback。

---

## 10. C ABI / バインディング戦略

### 10.1 公開 API（newdom.h）

```c
// --- Context ---
NdCtx*   nd_create(NdConfig* config);
void     nd_destroy(NdCtx* ctx);

// --- Scene Graph ---
NdNodeId nd_node_create(NdCtx* ctx, NdNodeKind kind);
void     nd_node_destroy(NdCtx* ctx, NdNodeId id);
void     nd_node_set_parent(NdCtx* ctx, NdNodeId id, NdNodeId parent);
void     nd_node_update(NdCtx* ctx, NdNodeId id, NdNodeProps* props);

// --- Frame ---
void     nd_begin_frame(NdCtx* ctx);
void     nd_end_frame(NdCtx* ctx);   // ここで GPU に流れる

// --- Text ---
NdTextId nd_text_shape(NdCtx* ctx, NdTextDesc* desc);
void     nd_text_destroy(NdCtx* ctx, NdTextId id);

// --- Events ---
NdNodeId nd_hit_test(NdCtx* ctx, float x, float y);

// --- Image ---
NdImageId nd_image_upload(NdCtx* ctx, void* data, uint32_t w, uint32_t h, NdPixelFormat fmt);
void      nd_image_destroy(NdCtx* ctx, NdImageId id);
```

### 10.2 言語バインディング戦略

**生成ではなく薄いラップ**。`newdom.h` の C ABI を各言語の FFI 機構で呼ぶ。

| 言語 | 接続方法 |
|---|---|
| TypeScript / JS | WASM exports / Emscripten bindings |
| Python | ctypes / cffi |
| Swift | Swift C interop |
| Kotlin / JVM | JNA / JNI |
| C++ | ヘッダー直接 include |
| Rust | bindgen から生成した raw bindings |
| Go | CGo |
| C# | P/Invoke |

### 10.3 TypeScript バインディング例

```typescript
// newdom.ts
import init, { NdContext } from "./newdom.wasm";

const nd = await init();
const ctx = nd.createContext({ backend: "webgpu", canvas });

const root = nd.createNode(ctx, "container");
const rect  = nd.createNode(ctx, "rect");
nd.setParent(ctx, rect, root);

nd.beginFrame(ctx);
  nd.updateNode(ctx, rect, {
    x: 10, y: 10, width: 200, height: 100,
    fill: { type: "solid", color: "#3b82f6" },
    cornerRadius: 8,
  });
nd.endFrame(ctx);
```

---

## 11. 盗める/参考にすべき先行技術

> パクれるものは全部パクる。新規発明は最小限に。

| コンポーネント | 参考元 | 何を盗むか |
|---|---|---|
| GPU Vector Renderer | **Vello** (Linebender) | GPU compute shader による 2D path rendering アルゴリズム |
| Text Shaping | **Harfbuzz** | そのまま組み込む（C ライブラリ）|
| Font System | **Fontique** (Linebender) | fallback chain、font enumeration |
| Scene Graph | **Flutter/Impeller** | Retained layer tree の設計 |
| Layout | **Yoga** (Meta) | Flexbox 実装をそのまま利用（MIT ライセンス）|
| WASM Bridge | **wgpu / Dawn** | WebGPU backend |
| Glyph Atlas | **cosmic-text** | GPU アトラス管理 |
| Constraint Layout | **Cassowary** | Constraint solver アルゴリズム |
| Dirty Tracking | **Slint** | Incremental repaint の設計 |

---

## 12. ロードマップ

### Phase 0 — Prototype（3ヶ月）

目標：WebGPU + WASM で矩形とテキストが描けること

- [ ] C コアの骨格（Scene Graph, Node CRUD）
- [ ] WebGPU バックエンド（WASM）
- [ ] 矩形描画（fill, stroke, cornerRadius）
- [ ] Harfbuzz 統合（Latin テキスト）
- [ ] TypeScript バインディング（最小限）
- [ ] ブラウザ上でのデモ

### Phase 1 — Usable（6ヶ月）

目標：実際の UI が作れること

- [ ] Flex レイアウト（Yoga 統合）
- [ ] 画像描画
- [ ] ベクターパス
- [ ] Hit testing / イベント領域
- [ ] Vulkan バックエンド（Linux/Android）
- [ ] Metal バックエンド（macOS/iOS）
- [ ] Python バインディング

### Phase 2 — Ecosystem（12ヶ月）

目標：フレームワークが NewDOM の上に乗れること

- [ ] **DOM Adapter 初版**（`newdom-dom` crate）— `createElement` / `appendChild` / `getElementById` / `addEventListener` / イベント合成・バブリング
- [ ] CJK / Bidi テキスト
- [ ] IME サポート
- [ ] Constraint レイアウト
- [ ] Layer compositing（opacity, blend mode, shadow）
- [ ] Animation primitive（tween, spring）
- [ ] Accessibility tree export（AX API / AT-SPI）
- [ ] DX12 バックエンド
- [ ] Reference framework 実装（TypeScript で薄い React-like）

### Phase 3 — Production（18ヶ月〜）

- [ ] OpenGL fallback
- [ ] Performance profiler / DevTools
- [ ] Variable font
- [ ] 絵文字完全対応
- [ ] WebAssembly Component Model 対応

---

## 13. 非目標（やらないこと）

| 項目 | 理由 |
|---|---|
| HTML / CSS 互換 | Document web は DOM に任せる |
| ブラウザ置き換え | スコープ外 |
| 状態管理 | 上層の仕事 |
| コンポーネントシステム | 上層の仕事 |
| ルーティング | 上層の仕事 |
| アクセシビリティの完全対応 | Phase 2 以降。初期は AX tree の export API のみ提供 |
| サーバーサイドレンダリング | SVG/PDF エクスポートとして別途検討 |

---

## 14. 設計上のトレードオフ

### 14.1 C vs Zig

| | C | Zig |
|---|---|---|
| FFI 互換性 | ◎ | ◎ |
| 安全性 | △ | ○（comptime, 明示的アロケータ）|
| WASM | ◎ | ◎ |
| エコシステム | ◎ | △（まだ若い）|
| **判断** | 初期は C。Zig への移行は後続判断。 |

### 14.2 Retained vs Immediate

Immediate mode（Dear ImGui 方式）は **採用しない**。

理由：
- 毎フレーム全 node を CPU で走査するコストが高い
- アニメーション・トランジションの管理が困難
- テキストの shaping キャッシュが効かない

Retained mode を採用し、差分更新で上位の利便性と下位の効率を両立する。

### 14.3 Shader の管理

各バックエンドで shader を別に書くのは現実的でない。  
**WGSL（WebGPU Shading Language）を正とし、他バックエンドへはトランスパイル**する。

```
WGSL（マスター）
  ↓ naga（wgpu のシェーダーコンパイラ）
Vulkan SPIR-V / Metal MSL / HLSL / GLSL
```

---

## 15. 参考資料

| 資料 | URL |
|---|---|
| Vello (GPU 2D renderer) | https://github.com/linebender/vello |
| Xilem (Rust UI framework) | https://github.com/linebender/xilem |
| Harfbuzz | https://harfbuzz.github.io |
| Yoga (Flexbox) | https://yogalayout.dev |
| wgpu (WebGPU in Rust) | https://wgpu.rs |
| cosmic-text | https://github.com/pop-os/cosmic-text |
| Cassowary (Constraint) | https://overconstrained.io |
| Flutter Impeller 設計 | https://github.com/flutter/flutter/wiki/Impeller |
| WebGPU Spec | https://gpuweb.github.io/gpuweb |

---

## Appendix A — Node Props 詳細

```c
typedef struct NdNodeProps {
    // Geometry
    float x, y, width, height;

    // Transform
    float rotate;       // degrees
    float scale_x, scale_y;
    float anchor_x, anchor_y; // 0.0 ~ 1.0

    // Rect
    float corner_radius[4];   // TL, TR, BR, BL
    NdBrush fill;
    NdStroke stroke;

    // Layer
    float opacity;
    NdBlendMode blend_mode;
    NdShadow shadow;
    NdFilter filter;       // blur, etc.

    // Text
    NdTextId text_id;      // nd_text_shape() の結果

    // Image
    NdImageId image_id;
    NdImageFit fit;        // cover, contain, fill, none

    // Clip
    bool clip_children;
    float clip_radius[4];

    // Visibility
    bool visible;
    NdPointerEvents pointer_events; // auto, none

    // Layout
    NdLayoutMode layout_mode;
    NdFlexProps flex;
    NdGridProps grid;
} NdNodeProps;
```

---

## Appendix B — エラー設計

```c
typedef enum NdError {
    ND_OK              = 0,
    ND_ERR_INVALID_ID  = 1,   // 存在しない node ID
    ND_ERR_GPU         = 2,   // GPU エラー（VRAM 不足等）
    ND_ERR_OOM         = 3,   // メモリ不足
    ND_ERR_TEXT        = 4,   // text shaping 失敗
    ND_ERR_BACKEND     = 5,   // backend 未初期化
} NdError;

// 全 API は NdError を返す。失敗時は nd_last_error() で詳細取得。
```

---

*NewDOM Specification v0.1 — 2026*  
*"Document web は HTML が作った。Application web は NewDOM が作る。"*
