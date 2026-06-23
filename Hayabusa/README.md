# Hayabusa（隼）

> **Hayabusa は、Hayate の Element Layer 上で動く Signal ベースの SFC フレームワークである。**

Hayabusa は、リアクティブランタイム（Signal グラフ・伝播・reconcile・スケジューリング・
DSL 式評価）を **Rust で単独所有**する。各言語の既存ランタイムを再利用する Tsubame と
対をなす、意図的な逆張りである（CONTEXT.md / [ADR-0001](docs/adr/0001-runtime-first-architecture-and-responsibility-line.md)）。

責務線：**再利用可能なものは Rust ランタイム、副作用の本体は script**。

```
.hybs  ──compile──▶  Template IR ──instantiate/bind──▶  ElementSink ──▶  hayate-core ElementTree
（後続）             （純粋式 binding）   （fine-grained patch）   （host-ABI 線）
                          ▲
                  自作 fine-grained リアクティブコア
                  （Signal / Computed / Effect）
```

## ステータス

| モジュール | 役割 | ADR |
|-----------|------|-----|
| [`reactive`](src/reactive.rs) | 自作 fine-grained コア（Signal / Memo / Effect、glitch-free、flush 合体）＋所有 Scope（teardown / cleanup） | 0003 |
| [`value`](src/value.rs) | 閉じた値モデル（number / string / bool / list / record） | 0003 |
| [`expr`](src/expr.rs) | 純粋式の AST と評価器（binding は純粋式） | 0004 |
| [`parse`](src/parse.rs) | 純粋式 DSL のパーサ（テキスト → `Expr`） | 0004 |
| [`template`](src/template.rs) | 手組み Template IR（要素・`:if`・`:each`・コンポーネント） | 0004 / 0006 |
| [`component`](src/component.rs) | コンポーネント定義・prop / emit / lifecycle（script 代役の setup） | 0004 |
| [`sink`](src/sink.rs) | `ElementSink` mutation サーフェス（`ElementTree` に 1:1 で写る host-ABI 線） | 0002 |
| [`instantiate`](src/instantiate.rs) | Template IR の instantiate / bind / 構造 reconcile / コンポーネント mount | 0004 / 0006 |
| [`hayate_sink`](src/hayate_sink.rs) | `ElementSink` を実 `hayate_core::ElementTree` へ転送する `HayateSink`（`feature = "hayate-core"`） | 0002 / 0009 |
| [`app_host`](src/app_host.rs) | 共有 App Host へ `DeliverySink` として mount する `HayabusaApp`（borrowed-tree ＋ ListenerId ルーティング・`feature = "app-host"`） | 0117 / 0009 |

### 通っているスライス

- **Slice 1（tracer bullet・ADR-0006）**：カウンタ例。**`count` increment 時にテキスト
  ノードだけが patch される** fine-grained patch を `tests/counter.rs` で実証。
- **Slice 2（構造 reconcile・ADR-0004）**：所有 Scope による teardown と、
  - `:if`：条件 signal で body を mount / unmount（兄弟混在でも anchor で正しく挿入、
    teardown で要素除去＋ブランチ Effect 破棄）
  - `:each`（keyed-only）：同一キーの値更新は **in-place patch**、追加/削除は行 Scope の
    生成/破棄、並べ替えは **move（再生成しない）**

  を `tests/control_flow.rs` で実証。
- **Slice 3（コンポーネント合成・ADR-0004）**：ランタイム上のコンポーネントインスタンス。
  - prop：親スコープの式 → 子の入力（Memo）への配線。親 signal 変化が子を fine-grained patch
  - emit：子イベント → 親ハンドラへのルーティング（payload 付き）
  - インスタンス隔離：各インスタンスが独立した signal Scope を持つ
  - lifecycle：`on_mount` / `on_destroy`（`:if` / `:each` の teardown に乗る）

  を `tests/component.rs` で実証。
- **Slice 4（式 DSL パーサ・ADR-0004）**：`.hybs` の束縛・`:if` 条件・`:each` key 式の
  フロントエンド。`Expr::parse("count + 1")` / `"item.label"` / `"n > 3 && !done"` を
  字句解析＋優先順位付きで `Expr` に落とす。`tests/parse_integration.rs` でパース済み式が
  binding / `:if` / `:each` を駆動することを実証。
- **Slice 5（実機コア統合・ADR-0009）**：`HayateSink` が `ElementSink` を実
  `hayate_core::ElementTree` へ 1:1 転送する。クロスワークスペースの path 依存リンクは
  `[patch.crates-io]` の複製で通る（spike 実証）。counter tracer bullet を実 `ElementTree` 上で
  通し、increment 時に text ノードが実 Element Layer で patch されることを `tests/hayate_sink.rs`
  で実証（`feature = "hayate-core"`）。
- **Slice 6（App Host 配線・ADR-0117）**：`HayabusaApp` を共有 `hayate_app_host::AppHost` へ
  `DeliverySink` として mount。**App Host が tree を所有する borrowed-tree モデル**で、
  reactive effect が積む mutation を buffering sink に溜め、`handle` がフレーム内で借用ツリーへ
  drain する（unsafe 不使用）。click は mount 時登録の `ListenerId → ElId → handler` で
  ルーティング。`tick → poll_deliveries → handle → flush → 借用ツリーへ patch` の 1 フレーム
  完全ループを `tests/app_host.rs` で実証（`feature = "app-host"`）。

含めない（後続）：`<template>` マークアップのパーサ＋`.hybs` 全体のコンパイラ、他言語
wasm ゲスト、router、Store、Resource、Click 以外のイベント（`on:input` 等は P4・ADR-0007）、
描画 present を伴う Platform Adapter 統合。

> デモアプリ到達のために決める必要がある未決 ADR 論点は
> [`docs/pending-decisions.md`](docs/pending-decisions.md) に記録している。

### 実際の hayate-core 駆動について

`ElementSink` は `hayate_core::ElementTree` の対応 API（`element_create` /
`element_set_text` / `element_append_child` / `set_root`）に 1:1 で写るよう設計してある。
tracer bullet では fine-grained patch を観測可能にする `RecordingSink` を使い、実際の
`ElementTree` へ転送する `HayateSink` は `feature = "hayate-core"` で有効化する。

クロスワークスペースのリンクは spike で検証済み（ADR-0009）：Hayabusa は Hayate とは別
ワークスペースで、hayate-core の `[patch.crates-io]`（vendored crate・Hayate ADR-0007）は
継承されない。`Cargo.toml` に同じ patch テーブルを複製すると path 依存リンクが通る。既定
ビルドは外部依存ゼロの self-contained（ADR-0006）のまま。

## 使い方

```bash
# 既定（self-contained tracer bullet）
cargo test    # 17 unit + 4 integration + 1 doctest
cargo clippy --all-targets -- -D warnings

# 実機コア統合（HayateSink で実 ElementTree を駆動）
cargo test --features hayate-core    # ＋ hayate_sink の unit/integration テスト

# App Host 配線（borrowed-tree モデルで 1 フレームの完全ループ）
cargo test --features app-host       # ＋ app_host の integration テスト
```
