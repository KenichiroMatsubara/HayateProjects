# 宣言モデル：Template IR をランタイムが instantiate し、binding は純粋式・コンポーネントは runtime インスタンス

status: accepted

## Template IR とランタイム instantiate

`.hybs` の `<template>` はコンパイルされて **Template IR**（要素ツリー・静的 prop・
reactive binding を記述するデータ構造）になる。**ランタイムがこの Template IR を
instantiate / bind し、hayate-core の ElementTree を駆動する**。

reconcile は diff / VDOM を持たず、**fine-grained デルタ**で行う。構造の reconcile
（要素の追加・削除・並べ替え）もランタイムが所有する。script が要素を手続き的に作るのではなく、
宣言を渡してランタイムが構造を所有する（ADR-0001 の責務線）。

## binding

binding（signal → element-prop）は**純粋式の AST ＋ target（element, prop）**で表す。
ランタイムが式を評価（signal を read）して prop を write する。最小の式評価器を
ランタイムに持つ。binding を script クロージャで表すことはしない（再利用不可・
言語固有になるため）。

副作用の**本体**（on-click 等のハンドラ）だけが closure（script。Rust が初手の代役）。
「純粋な束縛＝ランタイム、副作用の本体＝script」という線。

## コンポーネント合成（runtime インスタンス）

`.hybs` 1つ＝1コンポーネント。コンポーネントの合成は**ランタイム上のコンポーネントインスタンス**
として行う。各インスタンスは独自の signal スコープ（Scope・ADR-0003）・prop 入力・
emit 出力・lifecycle を持つ。

| 概念 | ランタイム（再利用可能） | script（言語固有） |
| ---- | ---- | ---- |
| prop | 親スコープの式 → 子の入力 signal への束縛配線 | `prop(...)` の宣言面 |
| emit | 子の emit → 親の登録ハンドラへのルーティング | `emit(...)` 呼び出し＋親ハンドラ本体 |
| 境界 | インスタンス化・signal スコープ隔離・lifecycle 駆動 | `on_mount` / `on_destroy` 本体 |

コンパイル時フラット化（B を A にインライン）は採らない。`:each` による動的個数の
コンポーネント生成・per-instance の状態とライフサイクルを潰してしまうため。

## `:each` は keyed-only

- 構文は **`:each ... by <式>` 一本**（keyed を既定、key 式を必須）
- index モードは別構文を設けず、**キー式 `by i`** として表現する
- item の値は常に（キー単位の）signal。よって**同一キーの値更新は再生成せず in-place patch**
  され、これが従来の Index 相当の「行は据え置き・値は reactive」挙動を再現する
- 並べ替えは **move** で各 item の Scope 状態（signal / 要素 / スクロール位置）を保持する
- キーは**式ベース**（参照 identity ではない）。閉じた値モデルは境界で marshal / コピー
  されるため、オブジェクト参照 identity は ABI 越しに不安定だから

## Consequences

- hot-reload は「新 Template IR への reconcile」として説明できる（ADR-0006）
- per-instance Scope があるため keyed-only が強制される（位置キーでは状態が漏れる）
- DSL 式評価器がランタイムにあることが binding / prop / key 式の共通基盤になる
