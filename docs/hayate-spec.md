# Hayate — 設計仕様書 v1.0

> **newdom-spec.md との差分**: プロジェクト名を Hayate に統一。公開インターフェースを C ABI（newdom.h / cbindgen）から WIT + wit-bindgen に移行（ADR-0015）。DOM Adapter（createElement 等）を廃止。Browser Extension ユースケースをスコープ外に整理。不可視 textarea を廃止し EditContext 専用に移行（ADR-0016）。Element Layer / Raw Layer の二層構造（ADR-0013）と Canvas Mode / HTML Mode（ADR-0016）を正式採用。各変更の経緯は `docs/adr/` を参照。旧仕様原文は `docs/archive/` に保存。

---

## 0. 哲学

> **"描くことを知らず、描く"**

Hayate はフレームワークではない。
Hayate は言語でもない。
Hayate は **描画の共通語** である。

上に何が乗ってもいい。TypeScript でも Python でも Swift でも Kotlin でも C でも、どんな言語の UI フレームワークも Hayate の上で動く。
下に何があってもいい。WebGPU でも Vulkan でも Metal でも DX12 でも、wgpu がその差異を吸収する。

Hayate が提供するのは二層の WIT インターフェースである。Element Layer でスタイル付き element tree を組み立て、Raw Layer で絶対座標の描画コマンドを直接制御する。Hayate 内部がレイアウト・スタイル解決・レンダリングを担い、上位層は「何を描くか」だけを伝えればよい。

DOM 互換は設計目標に含まない。

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
| AI チャット UI | ❌ ストリーミングでの DOM 操作コスト |
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

## 2. Hayate の定義

### 2.1 What Hayate IS

```
Hayate Core = GPU-Native Retained Scene Graph
            + Text/Vector Rendering Pipeline (Vello)
            + wgpu Backend
            + Element Layer（Taffy によるレイアウト + スタイル解決）
            + Raw Layer（絶対座標・描画コマンド直接受付）

Platform Adapter = IME 入力 + クリップボード + raw 入力イベント変換
                 （プラットフォームごとに異なる実装）
```

**Hayate Core は描画の "カーネル" である。**
OS のカーネルが言語を選ばず全てのプログラムの下にあるように、Hayate Core は UI フレームワークを選ばず全ての描画の下にある。

**Platform Adapter は Core とプラットフォームを仲介する。**
IME・クリップボード・raw 入力イベントをプラットフォーム固有 API から Hayate の統一インターフェースに変換する。Core は Adapter を知らない。

### 2.2 What Hayate IS NOT

- ❌ フレームワーク（React / Vue の代替ではない）
- ❌ 状態管理（Redux / Signal の代替ではない）
- ❌ HTML/CSS 互換実装
- ❌ 特定言語のライブラリ
- ❌ Document Model（DOM の置き換えではない）

### 2.3 Hayabusa との関係

**Hayabusa（隼）** は Hayate の Element Layer 上に構築された Signal 型 Rust フレームワークである。`view!` マクロ・`#[component]`・Signal / Memo / Effect・Router・Store・Resource を提供する。Hayabusa が Signal diff を取り、変化分を Hayate Element Layer に流す。Hayate は受け取って描くだけである。

```
┌─────────────────────────────────────────────────┐
│   Application（Rust / TypeScript / Python ...）  │
├─────────────────────────────────────────────────┤
│   Hayabusa（Signal フレームワーク、Rust 実装）    │
├─────────────────────────────────────────────────┤
│   Element Layer  ←→  WIT インターフェース         │
├─────────────────────────────────────────────────┤
│   Hayate Core（Rust）                            │
│   Scene Graph / Vello / wgpu                     │
├─────────────────────────────────────────────────┤
│   Raw Layer  ←→  WIT インターフェース            │
└─────────────────────────────────────────────────┘
```

他言語 SDK（TypeScript・Python・Go・C 等）は WIT から wit-bindgen で自動生成され、Element Layer 経由でスタイル付き UI を構築する。Raw Layer 経由で生座標を直接制御することもできる。

### 2.4 ポジショニング

```
┌──────────────────────────────────────────────────────┐
│   Application / Hayabusa / 他言語 SDK                │  ← 何でもよい
├──────────────────────────────────────────────────────┤
│              WIT（Element Layer / Raw Layer）         │  ← 公開 API 単一ソース
├──────────────────────────────────────────────────────┤
│                                                       │
│                H A Y A T E   C O R E                 │
│    (hayate-core + hayate-adapter-web / native ...)    │
│                                                       │
├──────────────────────────────────────────────────────┤
│        wgpu                                           │  ← GPU 抽象（固定）
├──────────────────────────────────────────────────────┤
│  WebGPU   Vulkan   Metal   DX12                      │  ← GPU バックエンド
└──────────────────────────────────────────────────────┘
```

---

## 3. アーキテクチャ

### 3.1 実装言語

**Rust**（ADR-0001）。

wgpu は Rust ネイティブのライブラリであり、Rust で統一することで cargo 一本に収まり、クロスコンパイル・Wasm・WIT コンパイルが一貫したツールチェーンで完結する。

### 3.2 公開インターフェース戦略（ADR-0015）

WIT（WebAssembly Interface Types）が Hayate の公開インターフェースの唯一のソースである。

- **Web 向け**: Wasm コンポーネントとしてコンパイル。ブラウザが提供する Wasm ランタイム上で動作する
- **ネイティブ向け**: wit-bindgen でネイティブライブラリとしてコンパイル。Wasm ランタイム（wasmtime 等）をホストプロセスに埋め込む必要がない

C ABI（cbindgen / newdom.h）は廃止済み。C ユーザーは引き続き C ヘッダーで使えるが、そのヘッダーは wit-bindgen-c が WIT から生成する。WIT が C ABI をラップするのではなく、WIT から C ABI が派生する。

### 3.3 WIT 二層構造（ADR-0013）

Hayate の公開 WIT インターフェースは二層からなる。

**Element Layer（上位）**
element tree の作成・Hayate CSS スタイルの設定・ツリー組み立てを受け付ける。Hayate 内部でレイアウト計算（Taffy）とスタイル解決を行い、Raw Layer コマンドに変換する。Hayabusa および他言語 SDK の標準的な利用層。

**Raw Layer（下位）**
絶対座標・確定スタイル済みの描画コマンドを直接受け付ける。`create-rect` / `create-text-run` / `create-image` / `create-clip` / `create-layer` 等の型付きコンストラクタで構成される。レイアウトを自前で計算するユースケース（ゲーム HUD・Infinite Canvas・カスタム layout engine）向けに公開する。Element Layer は内部でこの層に変換して使う。

両層とも WIT で外部公開する。

### 3.4 Element 語彙

Element Layer は React Native の語彙を採用する。LLM の訓練データ上で React Native・SwiftUI・Jetpack Compose の三系統に共通する語彙であり、文脈なしでも意味が一意になる。HTML タグ名（div / span / input 等）は使用しない。

| Element 型 | 説明 |
|---|---|
| `view` | 汎用コンテナ。レイアウト・クリップ・グループ化 |
| `text` | テキスト表示 |
| `image` | 画像表示 |
| `button` | タップ・クリック可能なコントロール |
| `text-input` | テキスト入力フィールド（IME 対応） |
| `scroll-view` | スクロール可能なコンテナ |

### 3.5 Hayate CSS

Hayate 固有のスタイルシステム。CSS 互換実装ではなく、CSS 命名を採用した Hayate 固有の仕様である。

- **レイアウトプロパティ**（display / gap / align-items / justify-content / grid-template-columns 等）: Taffy の CSS Flexbox / Grid / Block 実装を仕様書とする
- **ビジュアルプロパティ**（color / background-color / border-radius / opacity 等）: CSS プロパティ名を踏襲しつつ Hayate が対応サブセットを定義する

getComputedStyle() + getBoundingClientRect() によるブラウザ計算結果の抽出は採用しない。reflow コストが消えず、ペイントを置き換えるだけでは性能改善にならないため（ADR-0010, ADR-0011）。

### 3.6 crate 構成

| crate | 役割 |
|---|---|
| `hayate-core`（`crates/core`） | Scene Graph（NodeId, NodeKind, SceneGraph）。Vello + wgpu。レイアウト（Taffy）。テキストスタック（Parley）。Element Layer / Raw Layer の WIT 実装。wasm-bindgen 依存なし |
| `hayate-adapter-web`（`crates/adapters/web`） | Web Platform Adapter。wasm-bindgen。Canvas Mode（WebGPU + EditContext）と HTML Mode の切り替え。IME / クリップボード / raw 入力変換 |

将来的に `hayate-adapter-macos` / `hayate-adapter-windows` / `hayate-adapter-linux` 等が追加される。Platform Equal Tier（ADR-0012）の原則により、各 Adapter は一級実装として設計される。

### 3.7 Platform Adapter の責務（ADR-0014）

Platform Adapter が担う責務は以下の三つに限定される。

1. **IME 入力**: composition-start / composition-update / composition-end / commit-text を WIT インターフェース経由で Core に通知する
2. **クリップボード**: プラットフォーム固有のクリップボード API を WIT インターフェース経由で抽象化する
3. **raw 入力イベント変換**: プラットフォーム固有のポインタ・キーボードイベントを Hayate の統一イベント型に変換する

以下は Platform Adapter の責務に含まない。

| 責務 | 担う主体 |
|---|---|
| サーフェス生成 | wgpu |
| フレームタイミング | wgpu / winit |
| アクセシビリティ報告 | AccessKit（Core 組み込み） |

Core は Adapter を知らない。新プラットフォームの追加コストは三責務の実装に限定される。

### 3.8 スレッドモデル

シングルスレッド（ADR-0003）。wgpu の `!Send` 型と Wasm 環境の制約により、現在はシングルスレッドで設計する。`hayate-core` は `!Send + !Sync`。レンダースレッド分離は API 安定後の将来 ADR として予約。

---

## 4. Web Platform Adapter（hayate-adapter-web）

### 4.1 Canvas Mode と HTML Mode（ADR-0016）

`hayate-adapter-web` はランタイム自動検出により Canvas Mode と HTML Mode を切り替える。アプリ側はモードを意識しない。

**Canvas Mode**
- 条件: WebGPU（`navigator.gpu`）と EditContext API の両方が利用可能な場合
- 描画: Vello + wgpu（WebGPU）で全 UI を Canvas に GPU 描画
- IME: EditContext API を使用
- 現時点では Chromium 系ブラウザが該当

**HTML Mode**
- 条件: WebGPU または EditContext API のいずれかが利用できない場合
- 描画: Canvas Mode と同一の統一パイプライン（Element Layer → Taffy → Raw Layer）を経由し、Raw Layer の絶対座標出力を absolutely-positioned な `div` にマッピングして描画する。Canvas Mode との差異は最終的な描画先のみ（DOM vs GPU）
- IME: ブラウザ native の動作に委ねる
- Canvas Mode との差異はアダプターの最終出力のみ。コアのパイプラインは共通

不可視 `<textarea>` + compositionEvent による IME 実装は廃止済み（ADR-0016）。

### 4.2 Platform Equal Tier（ADR-0012）

Hayate は「Web 最優先」ではなく「Web が最初の実装、全プラットフォームが品質で等階級」という原則を採用する。

- 実装順序: Web → macOS / Windows / Linux → iOS / Android
- アーキテクチャ上の優遇: なし。Core は Platform Adapter を知らず、wgpu が GPU surface の差を吸収し、WIT が言語・プラットフォーム非依存の契約を定義する

---

## 5. Scene Graph 仕様

### 5.1 Node 型

Hayate の Scene Graph は Raw Layer が管理する描画プリミティブで構成される。GPU が直接処理できる型のみ存在する。

| Node 型 | 説明 |
|---|---|
| `rect` | 矩形（座標・サイズ・色・角丸） |
| `text-run` | テキストラン（グリフ列） |
| `image` | 画像（image_id・fit） |
| `clip` | クリップ領域 |
| `layer` | レイヤー（opacity・blend_mode） |

### 5.2 NodeId

`slotmap::DefaultKey`（generational arena）。削除済み Node への誤 mutation は generational check で検出され、安全に無視される。上位層は「どの entity が どの NodeId か」のマッピングを自身で管理する。

### 5.3 Retained Graph の更新モデル

Hayate は Retained（保持型）方式を採用する。Scene Graph が前フレームの状態を保持し、上位層は変更のあった Node のみを通知する。

```
初回: node_create(kind) -> NodeId
更新: node_update(id, changed_props)  // 変更分のみ
削除: node_destroy(id)
移動: node_set_parent(id, parent_id)
```

Hayate は Mutation を受け取るだけで、状態変化を自ら検知しない。

---

## 6. 描画パイプライン

### 6.1 フレームループ

```
1. begin_frame()
   └── Mutation 受付開始

2. node_update() × N
   └── Scene Graph を更新

3. end_frame()
   └── SceneGraph → Vello Scene 変換
   └── Vello: GPU compute shader で path rendering
   └── 中間テクスチャ (Rgba8Unorm) → surface blit
   └── surface.present()
```

### 6.2 SceneGraph → Vello Scene 変換レイヤー

Vello の API 変更を Hayate コアから隔離するための薄いレイヤー（ADR-0006）。Scene Graph を深さ優先で走査し、Node 型に対応する Vello プリミティブへ変換する。この変換関数のみが Vello の型に触れる。

### 6.3 Dirty Region

現時点では全画面再描画を許容する。Dirty Region による部分再描画は将来の最適化。ただし Retained Scene Graph により text shaping / layout 計算 / glyph atlas は常にキャッシュされる。

---

## 7. GPU Backend

wgpu を唯一の Backend として使用する（ADR-0002）。独自の Backend 抽象は持たない。

| 環境 | wgpu backend |
|---|---|
| Web (Wasm) | ブラウザ WebGPU |
| Android | Vulkan |
| iOS / macOS | Metal |
| Windows | DX12 / Vulkan |
| Linux | Vulkan |

---

## 8. テキストエンジン

Linebender スタック（ADR-0005）。Vello と同チーム設計で自然に統合される。

| crate | 役割 |
|---|---|
| `parley` | text layout（行折り返し、alignment、paragraph） |
| `fontique` | font management（fallback chain、font enumeration） |
| `skrifa` | font parsing（OpenType） |

---

## 9. Layout Engine

Element Layer 内部で Taffy を使用する（ADR-0004）。

- Taffy（Pure Rust）を採用。Flexbox + CSS Grid + Block layout
- `hayate-core` が Taffy を内部で使用し、Element Layer → Raw Layer 変換の一部として動作する
- Raw Layer を直接使うユーザー（ゲーム HUD・Infinite Canvas 等）は Taffy を経由しない

---

## 10. 依存管理

主要依存は workspace 内にベンダリングし upstream から自律する（ADR-0007）。

| crate | ベンダー場所 | 理由 |
|---|---|---|
| vello / vello_encoding / vello_shaders | `crates/vendor/vello` 等 | 描画パイプラインの核心 |
| taffy | `crates/vendor/taffy` | レイアウト計算の核心 |
| parley / fontique / skrifa | `crates/vendor/parley` 等 | テキストスタックの核心 |

wgpu は対象外。巨大すぎ、プラットフォーム対応の追従コストが高い。

---

## 11. ロードマップ

### Step 1 — HTML Mode（WIT インターフェース確立）

目標: **HTML Mode を動かし、WIT インターフェース全体と Hayabusa との境界を確立する**

- Element Layer の WIT 定義（全 6 型: `view` / `text` / `image` / `button` / `text-input` / `scroll-view`）
- Raw Layer の WIT 定義（`create-rect` / `create-text-run` / `create-image` / `create-clip` / `create-layer`）
- `hayate-core` に Element Layer モジュール追加（element ツリー管理 + Taffy レイアウト）
- Element Layer → SceneGraph（Raw Layer）接続
- HTML Mode アダプター（SceneGraph 絶対座標 → absolutely-positioned `div`）
- ライフサイクル WIT export（`hayate-init` / `render` / `poll-events`）
- イベント WIT export（`poll-events` 統一キュー: pointer / focus / text-input / IME）
- Hit testing
- Hayabusa との接続（Signal diff → Element Layer mutation → `poll-events`）
- HTML Mode ブラウザデモ

### Step 2 — Canvas Mode

目標: **HTML Mode と同一パイプラインの最終段を Vello + WebGPU に差し替え、GPU 描画を動作させる**

- wgpu + Vello 初期化（Wasm）
- SceneGraph → Vello Scene 変換レイヤー（HTML Mode アダプターと差し替え）
- ランタイム自動検出（Canvas Mode / HTML Mode 切り替え）
- EditContext API による IME（Canvas Mode）
- テキスト描画（Parley + Vello glyph rendering）
- 画像描画
- Canvas Mode ブラウザデモ

### Step 3 — 多言語 SDK + ネイティブ

目標: **WIT インターフェースを他言語 SDK に展開し、ネイティブ Adapter を追加する**

- wit-bindgen による多言語 SDK 生成（TypeScript / C / C++ / Go 等）
- C / C++ から Hayate Element Layer を使えることの理論的確認と動作検証
- CJK / Bidi テキスト・IME
- Layer compositing（opacity、blend_mode、shadow）
- Animation primitive（tween、spring）
- Accessibility tree export（AccessKit）
- macOS / Windows / Linux ネイティブ Adapter 初版

---

## 12. 非目標（やらないこと）

| 項目 | 理由 |
|---|---|
| HTML/CSS 互換実装 | DOM と Hayate は別のモデル。互換は設計目標でない |
| getComputedStyle による CSS 計算結果の抽出 | reflow コストが消えない（ADR-0010, ADR-0011） |
| DOM Adapter（createElement / addEventListener 等） | 廃止済み。WIT が唯一の公開インターフェース |
| Browser Extension ユースケース | スコープ外 |
| 不可視 textarea による IME | 廃止済み（ADR-0016）。EditContext 専用 |
| 独自 C ABI（newdom.h / cbindgen） | 廃止済み（ADR-0015）。WIT から wit-bindgen-c が生成 |
| 状態管理 | 上層（Hayabusa 等）の仕事 |
| コンポーネントシステム | 上層（Hayabusa 等）の仕事 |
| ルーティング | 上層（Hayabusa 等）の仕事 |
| 独自 GPU Backend 抽象 | wgpu がその役割を担う（ADR-0002） |
| 独自 GPU compute path renderer | Vello がその役割を担う（ADR-0006） |
| サーバーサイドレンダリング | SVG/PDF エクスポートとして別途検討 |

---

*Hayate Specification v1.0 — 2026*
*"Document web は HTML が作った。Application web は Hayate が描く。"*
