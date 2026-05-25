# インタラクション状態はイベント通知のみ。スタイル切替は上位層の責務

## Context

ポインタ操作やキーボード操作によるインタラクション状態（hover / active / focus 等）に応じてスタイルを変えるUI表現は一般的である。CSS では `:hover` / `:active` / `:focus` 擬似クラスとしてレンダラー側が担う。

Hayate がこれを実現する方法として二つの候補があった。

1. **render-layer 方式**: `StyleProp` に状態付きスタイルを追加する（例: `element_set_hover_style(id, props)`）。Hayate がポインタ位置を監視し、要素がホバー状態かを判定してスタイルを切り替える
2. **event-driven 方式**: Hayate は `hover-enter` / `hover-leave` / `active-start` / `active-end` 等のイベントを `poll-events()` に追加するだけ。スタイル切替は上位層（Hayabusa の Signal / Effect）が担う

## Decision

**event-driven 方式を採用する。**

Hayate はインタラクション状態に応じたスタイルという概念を持たない。Hayate が担うのは「どの要素でイベントが発生したか」を上位層に通知することだけである。`:hover` 状態でのスタイル切替は Hayabusa が Signal を使って実装する。

```rust
// Hayabusa 側のイメージ
let is_hovered = create_signal(false);
on_event(EventKind::HoverEnter, move |_| is_hovered.set(true));
on_event(EventKind::HoverLeave, move |_| is_hovered.set(false));
element_set_style(id, if is_hovered.get() { hover_style } else { base_style });
```

## Considered Options

- **render-layer 方式（却下）**: Hayate がホバー判定・スタイル解決・再描画トリガーを持つ。ADR-0018 で確立した export poll モデルの下では、Hayate はイベントをキューに貯めるだけであり、Hayate 自身がスタイルを能動的に切り替えるアーキテクチャは一方向依存の原則と衝突する。またアニメーション付きトランジション（ホバー時にフェードイン等）を後から加えるたびに WIT を拡張する必要が生じる
- **event-driven 方式（採用）**: Hayate は状態を持たない。スタイル切替は Hayabusa の Signal / Effect が担い、Hayabusa が毎フレーム `element_set_style` で渡す。Hayate はその値を受け取って描くだけ。Hayabusa 側でトランジション・イナーシャ等の演出を自由に実装できる

## Consequences

- `poll-events()` に `hover-enter` / `hover-leave` / `active-start` / `active-end` イベントを追加する
- Hayate の WIT に「状態付きスタイル」型は存在しない
- 上位層の実装によってはホバー判定に最大 1 フレームのラグが生じるが、人間に知覚不能
- Hayabusa はポインタイベントから Signal を更新し、次の `render()` 呼び出しまでにスタイルを確定させる
