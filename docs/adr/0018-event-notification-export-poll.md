# イベント通知は export poll モデルを採用する

## Context

Hayate Element Layer がイベント（click / focus / text-input / IME 等）を上位層（Hayabusa および他言語 SDK）に通知する方式を決定する必要がある。

候補は二つあった。

1. **import callback モデル**: Hayate が WIT import として上位層のコールバック関数を呼ぶ。Hayate がイベントを上位層に push する
2. **export poll モデル**: Hayate がイベントをキューに貯め、上位層が `poll-events()` export を呼んで取り出す

Hayate は言語非依存の最下層基盤（ADR-0012）であり、Hayabusa（Rust、別リポジトリ）だけでなく TypeScript・C・Python 等の SDK も上位層として想定される。

## Decision

**export poll モデルを採用する。**

Hayate は WIT import を可能な限り持たない。Hayate は export し、上位層が Hayate をインポートして使う。これが最下層基盤としての一方向依存の原則である。

全イベント種別（click / focus / scroll / composition-start / composition-update / composition-end / commit-text 等）を単一の `poll-events()` export に統一する。IME イベントを別扱いにしない。

```wit
// 上位層が呼び出す Hayate の export
export poll-events: func() -> list<event>;
```

## Considered Options

- **import callback モデル（却下）**: Hayate が上位層のコールバックを import して呼ぶ方式。Hayate が上位層の実装に依存することになり、「下位層は上位層を知らない」という基盤の原則に反する。上位層の言語・実行モデルごとにコールバックの振る舞いが異なる問題も生じる
- **export poll モデル（採用）**: Hayate は純粋に export のみ持つ。上位層がフレームループ内で `poll-events()` を呼んで取り出す。Wasm コンポーネントとしての依存方向が一方向に保たれる。UI イベントのポーリングレイテンシは最大 1 フレーム（≤16ms）であり、人間に知覚不能。非同期 I/O イベント（WebSocket 等）は Hayate を経由しないため即時性の要件と矛盾しない

## Consequences

- Hayate の WIT 定義は原則として export のみで構成される
- 上位層がフレームループを駆動する責任を持つ（`render()` 呼び出し → `poll-events()` 呼び出し）
- 上位層の実行モデル（Rust の async、TypeScript の event loop 等）に Hayate は一切依存しない
- IME イベントを含む全イベントが同一キューに入るため、上位層でのイベント処理順序が保証される
