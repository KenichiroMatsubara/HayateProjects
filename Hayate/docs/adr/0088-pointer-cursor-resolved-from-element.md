# ポインタカーソルは要素から解決し `on_pointer_move` で Platform Adapter に渡す（`cursor` 語彙）

**Status: accepted**

**Date: 2026-06-13**

> 本 ADR は 2026-06-13 に実装着地済みの決定（`feat(vocab): add cursor style tag and
> on_pointer_move resolution pipeline`）を遡及的に記録する。番号 0088 はコード
> （`style.rs` / `tree.rs` / `interaction.rs` / `canvas.rs`）が `ADR-0088` として
> 参照済みだったが ADR ファイルが欠落していた。

## Context

Canvas Mode には DOM が無く、CSS の `cursor` を要素ごとにブラウザ/OS へ効かせる経路が
無い。一方ポインタの hover/move は既に「ポインタ直下の要素」を hit-test しており
（ADR-0019 / Interaction State）、そこから cursor を解決して Platform Adapter に渡せば、
スタイルに触れずに OS/ブラウザのカーソルを駆動できる。DOM Mode ではブラウザの CSS
`cursor` に直接写像すれば足りる。

## Decision

1. **語彙追加（Protocol Contract）** — `style_tags.json` に `CURSOR`
   （`enum:cursor` = `default | pointer | text | crosshair | not-allowed | grab |
   grabbing`）を追加し、`CursorValue` として解決する。閉じた語彙（ADR-0071）の一員。

2. **要素から解決し move の出力に載せる** — `on_pointer_move` がポインタ直下の要素から
   cursor を解決し、`PointerMoveResult { moved, resolved_cursor }` で返す。layout 未準備や
   1px dedup で coalesce された move では再計算せず、直近値（`last_cursor`）を持ち越す。

3. **適用は Platform Adapter の責務** — Adapter は `resolved_cursor` から OS/ブラウザの
   カーソルを駆動し、要素スタイルには触れない（ADR-0014 の Adapter 責務に沿う）。Web
   Canvas adapter は生成済みの Hayate-CSS → browser-CSS マッパー（ADR-0070）を再利用して
   `document.body.style.cursor` に適用し、`cursor` 値リストを単一正本に保つ。

4. **DOM Mode** — `cursor` は生成マッパー経由で CSS `cursor` に直接写像する。

## Consequences

- カーソルは「要素から解決した出力」であり、要素ごとに push する設計ではない。Adapter は
  ビューポート単位で1つの cursor（body）を適用する。
- Canvas / DOM が同一の生成マッパーを共有するため、値リストのパリティが保たれる。
- coalesce された move では cursor を再計算しない（`last_cursor` を持ち越す）ので、
  ホバー中の無駄な解決を避ける。
