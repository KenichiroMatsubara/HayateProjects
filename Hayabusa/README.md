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
| [`expr`](src/expr.rs) | 最小の純粋式評価器（binding は純粋式） | 0004 |
| [`template`](src/template.rs) | 手組み Template IR（要素・`:if`・`:each`・コンポーネント） | 0004 / 0006 |
| [`component`](src/component.rs) | コンポーネント定義・prop / emit / lifecycle（script 代役の setup） | 0004 |
| [`sink`](src/sink.rs) | `ElementSink` mutation サーフェス（`ElementTree` に 1:1 で写る host-ABI 線） | 0002 |
| [`instantiate`](src/instantiate.rs) | Template IR の instantiate / bind / 構造 reconcile / コンポーネント mount | 0004 / 0006 |

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

含めない（後続）：`.hybs` パーサ / コンパイラ、他言語 wasm ゲスト、router、Store、Resource、
`HayateSink`（実 ElementTree 駆動）。

### 実際の hayate-core 駆動について

`ElementSink` は `hayate_core::ElementTree` の対応 API（`element_create` /
`element_set_text` / `element_append_child` / `set_root`）に 1:1 で写るよう設計してある。
tracer bullet では fine-grained patch を観測可能にする `RecordingSink` を使い、実際の
`ElementTree` へ転送する `HayateSink`（hayate-core への path 依存）は薄い後続実装になる。

## 使い方

```bash
cargo test    # 17 unit + 4 integration + 1 doctest
cargo clippy --all-targets -- -D warnings
```
