# box positioning は `relative`（既定）/ `absolute` の2値、insets で位置づけ

**Status: accepted（issue #205）**

**Date: 2026-06-13**

> 本 ADR は 2026-06-13 に実装着地済みの決定（`feat(vocab): add position:
> relative|absolute + top/left/right/bottom to style_tags`）を遡及的に記録する。番号
> 0091 はコード（`style.rs` / `tests/position_layout.rs`）が `ADR-0091, issue #205`
> として参照済みだったが ADR ファイルが欠落していた。

## Context

閉じた語彙（ADR-0071）には要素を通常フローから外して配置する手段が無かった。レイアウト
エンジンは Taffy（ADR-0004）で、その `position` 既定は `relative`（CSS の `static` 既定
とは異なる）。語彙はこのエンジンの実挙動に合わせる。

## Decision

1. **語彙追加** — `position: relative | absolute`（`PositionValue`）と insets
   `top` / `right` / `bottom` / `left` を `style_tags.json` に追加する。

2. **既定は `relative`** — Taffy の既定に一致させる。CSS の `static` 既定とは異なるが、
   レイアウトエンジンの実挙動を正本とする（語彙のモジュール完結・ADR-0083）。

3. **`absolute` は通常フローから外す** — `absolute` 要素は normal flow から外れ、
   `top`/`left`/`right`/`bottom` の insets で positioned ancestor に対して位置づけられる。
   フロー内の兄弟は、その absolute 要素が居ないものとして再レイアウトされる。

4. **`sticky` / `fixed` はスコープ外** — Taffy が対応しないため語彙に含めない。

## Consequences

- 既定が `relative` である点は CSS（`static` 既定）との明示的な乖離。レイアウトエンジン
  （Taffy）の実挙動を正本とする方針の帰結であり、ドキュメントで周知する。
- `sticky` / `fixed` は提供しない。将来 Taffy が対応した時点でモジュール完結原則
  （ADR-0083）に従って一括追加を検討する。
- insets は positioned ancestor 基準。positioned ancestor が無い場合の基準は Taffy の
  解決に委ねる。
