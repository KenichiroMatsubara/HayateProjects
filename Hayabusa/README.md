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

## ステータス：tracer bullet（[ADR-0006](docs/adr/0006-monorepo-crate-placement-and-hot-reload-as-reconcile.md)）

最初の vertical slice として、カウンタ例を自作部品だけで通している。
**`count` の increment 時にテキストノードだけが patch される**ことを `tests/counter.rs` で実証する。

| モジュール | 役割 | ADR |
|-----------|------|-----|
| [`reactive`](src/reactive.rs) | 自作 fine-grained コア（Signal / Memo / Effect、glitch-free、flush 合体） | 0003 |
| [`value`](src/value.rs) | 閉じた値モデル（number / string / bool / list / record） | 0003 |
| [`expr`](src/expr.rs) | 最小の純粋式評価器（binding は純粋式） | 0004 |
| [`template`](src/template.rs) | 手組み Template IR | 0004 / 0006 |
| [`sink`](src/sink.rs) | `ElementSink` mutation サーフェス（`ElementTree` に 1:1 で写る host-ABI 線） | 0002 |
| [`instantiate`](src/instantiate.rs) | Template IR の instantiate / bind / fine-grained patch | 0004 / 0006 |

含めない（後続）：`.hybs` パーサ / コンパイラ、他言語 wasm ゲスト、router、Store、Resource。

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
