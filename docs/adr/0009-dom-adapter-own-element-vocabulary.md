---
status: superseded — DOM 互換を設計目標から除外したため DOM Adapter ごと廃止。
---

# DOM Adapter の型文字列は HTML タグ名を採用し、フォーム系要素は初版で未サポートとする

DOM Adapter の `createElement(type)` が受け取る型文字列は HTML タグ名（`"div"`, `"span"`, `"section"` 等）をそのまま使う。独自語彙（`"view"`, `"rect"` 等）は採用しない。

理由：WebGPU が利用できない環境で DOM Adapter を本物の DOM にフォールバックさせる実装がタグ名 1:1 で対応でき、アプリケーションコードを変えずに済む。採用摩擦も下がり、既存の JSX ベースフレームワークとの親和性も高い。

構造タグ（`div`, `span`, `p`, `h1`〜`h6`, `section`, `article`, `header`, `footer`, `main`, `ul`, `li` 等）は HTML との意味論的乖離が小さく、コンテナとして素直にマップできる。

フォーム系要素（`input`, `button`, `select`, `textarea`, `form`）は HTML 仕様上の挙動（フォーム送信・バリデーション・type 属性ごとの分岐等）が複雑なため、初版では未実装とする。理論的には DOM Adapter で完全に実装可能であり、これはスコープ外ではなく実装優先度の問題である。

## Considered Options

- **DOM Adapter 独自語彙**（`"view"`, `"rect"` 等）: コアの NdNodeKind に近く混乱が少ないが、WebGPU→DOM フォールバックが複雑になり、既存フレームワークとの親和性も下がる。
- **HTML タグ名をそのまま採用**: フォールバックが 1:1 で対応でき、採用コストが低い。フォーム系の意味論的ずれはスコープを明示することで管理する。
