# はみ出しの扱い: box クリップ（`overflow`）と text truncation（`max-lines` がトリガ）

**Status: accepted（issue #206 / #207）**

**Date: 2026-06-13**

> 本 ADR は 2026-06-13 に実装着地済みの2決定（`feat(vocab): add overflow:
> hidden|visible with rounded-corner clipping (#206)` と `feat(vocab): add max-lines +
> text-overflow text truncation module (#207)`）を遡及的に記録する。番号 0090 はコード
> （`style.rs`）が `ADR-0090/issue #206` および `ADR-0090/issue #207` として参照済み
> だったが ADR ファイルが欠落していた。

## Context

閉じた語彙（ADR-0071）には子のはみ出しをクリップする手段も、テキストを行数で打ち切る
手段も無かった。CSS では box の `overflow` と、テキストの省略（`overflow` +
`white-space` + `text-overflow` の組合せ）が別系統だが、Hayate はレイアウトモジュール
完結の原則（ADR-0083）に沿って2つのモジュールとして揃え、テキスト省略のトリガは
`max-lines` 単一に簡約する。

## Decision

### box クリップ（#206）

`overflow: visible | hidden` を追加する（`OverflowValue`）。`visible` を既定とし、CSS の
`overflow` 既定（visible）に一致させる。`hidden` は子を要素の border box（角丸があれば
丸めた形状）にクリップする。

### text truncation（#207）

text truncation モジュールとして `max-lines`（`u32`）と
`text-overflow: clip | ellipsis`（`TextOverflowValue`、既定 `clip`）を追加する。

- **`max-lines` が唯一の打ち切りトリガ**。`text-overflow` は `max-lines` が設定されて
  いない限り効果を持たない。
- `clip` は `max-lines` を超えたテキストを黙って切り捨てる。`ellipsis` は最後の可視行に
  `…` を付加する。

いずれも生成マッパー（ADR-0070）経由で DOM の CSS に写像し、両レンダラーのパリティ検証
（hayate-css-parity / golden frame）対象とする。

## Consequences

- `max-lines` を唯一のトリガとする簡約は、CSS の「1行オーバーフロー時の text-overflow」
  とは異なる。語彙を小さく保つための意図的な乖離であり、必要になれば別スライスで深める。
- `overflow: hidden` のクリップは角丸（`border-radius`）と整合する形状で行う。
- grid の item 配置同様、本モジュール外のはみ出し制御（`overflow-x`/`overflow-y` 個別、
  `scroll` 値など）は語彙外。`scroll-view`（ADR-0022 / 0046）が別途スクロールを担う。
