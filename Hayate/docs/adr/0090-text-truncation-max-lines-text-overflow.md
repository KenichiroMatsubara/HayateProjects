# テキスト切り捨ては max-lines + text-overflow の 2 プロパティモジュールとする

**Status: accepted**

**Date: 2026-06-13**

## Context

単一行・複数行のテキスト末尾省略（`…`）は UI の基本要件である。CSS ではこれを実現するために以下の三点セットが慣習的に使われる:

```css
white-space: nowrap;
overflow: hidden;
text-overflow: ellipsis;
```

複数行版は `-webkit-line-clamp`（現 `line-clamp`）として別途定義されている。

Hayate に同じ CSS 語彙を持ち込む場合、ADR-0083（モジュール完結原則）により `white-space` モジュール（normal/nowrap/pre/pre-wrap/…）と `overflow` モジュール（hidden/scroll/visible/auto/…）を丸ごと引き連れることになる。どちらも独立したモジュールとして大きい。

代替として `line-clamp: N` 単独プロパティも検討した。

## Decision

`max-lines: N`（正整数）と `text-overflow: ellipsis | clip` の **2 プロパティモジュール** として追加する。

- `text-overflow` は `max-lines` が設定されているときのみ有効。`max-lines` なしでは切り捨て境界が生まれないため `text-overflow` は無意味。
- `max-lines` のみ指定した場合のデフォルトは `text-overflow: clip`（サイレントクリップ）。`…` を表示したい場合は `text-overflow: ellipsis` を明示する。
- N=1 で単一行省略、N>1 で複数行省略を兼ねる。
- `white-space` は語彙に追加しない（`max-lines: 1` が単一行制約を内包する）。
- コンテナの `overflow: hidden` はテキスト切り捨てとは別モジュール（ADR-0091 参照）。

Parley（テキストレイアウトエンジン）は `max_lines` パラメータを受け取れるため、このモデルは Parley の API に直線的にマッピングされる。

## Consequences

- CSS の `text-overflow` と字義がずれる（CSS では `overflow: hidden` が有効化トリガー、本 ADR では `max-lines` が有効化トリガー）。CSS 知識を持つ開発者に対してドキュメントで明示する必要がある。
- `white-space: pre` など折り返し制御の他の値（コードブロック表示等）は本 ADR の範囲外。必要になれば別モジュールとして追加する。
