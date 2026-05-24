# DOM Adapter は独自の型文字列語彙を持ち、HTML タグ名を使わない

DOM Adapter の `createElement(type)` が受け取る型文字列は、HTML タグ名（`"div"`, `"span"`, `"input"` 等）ではなく、DOM Adapter 固有の語彙（`"view"`, `"rect"`, `"text"`, `"image"`, `"canvas"` 等）とする。HTML タグ名との変換は、DOM Adapter の上に乗るさらなる adapter 層（HTML adapter）の責務とする。

## Considered Options

- **HTML タグ名をそのまま使う**: `createElement("div")` が動くため既存 Web フレームワークとの親和性が高いが、HTML Parser や CSS セレクターの意味論（`div` は block、`span` は inline 等）を DOM Adapter が解釈する必要が生じ、スコープが膨らむ。
- **DOM Adapter 独自語彙**: GPU primitive に近い名前（`"rect"`, `"view"` 等）を使う。HTML フレームワーク向けには上位の変換層を置けばよく、DOM Adapter コアをシンプルに保てる。

## Consequences

`createElement("div")` を直接呼んでも動かない。React DOM 等の既存ライブラリを NewDOM 上で動かすには、HTML タグ → DOM Adapter 語彙への変換を行う別 crate が必要になる。
