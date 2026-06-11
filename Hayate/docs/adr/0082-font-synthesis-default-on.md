# 実フェイスが無い weight / style はブラウザ準拠で合成する（font-synthesis 既定 ON）

**Status: accepted**

**Date: 2026-06-11**

## Context

Canvas のバンドルフォントは Noto Sans JP（variable）1ファイルのみ（ADR-0073）。italic フェイスも slant 軸も存在せず、`font-style: italic` は実フェイス方式では原理的に表現できない。一方ブラウザの `font-synthesis` は既定で weight / style とも合成 ON のため、DOM モードでは擬似斜体・擬似ボールドが出る。Canvas が合成しないとレンダラー間の意味論パリティ（system-wide ADR-0002）に違反し、「指定したのに何も起きない」沈黙故障になる。

## Decision

フォントの weight / style 解決は次の順とする:

1. **実フェイス / variable 軸**で表現できるならそれを使う（weight は `wght` 軸を含む）
2. 無ければ**合成**する — faux italic はグリフランへの skew（約14度）、faux bold は embolden

fontique のフォント選択が返す `Synthesis` 情報（embolden 要否・skew 角）を Parley の run からレンダラーへ流し、tiny-skia / vello の両バックエンドで適用する。

## Considered Options

- **合成しない**（表現できない指定は no-op、DOM 側に `font-synthesis: none` を付与してパリティ維持）: パリティは保てるが「italic と書いても何も起きない」が仕様になり、Web の既定から乖離する。却下。
- **italic フェイスのバンドル追加**: Noto Sans JP に公式 italic が存在せず、バイナリサイズも増える。却下。
- 将来 `fontSynthesis` を語彙に追加すれば opt-out を CSS 標準の形で提供できる。
