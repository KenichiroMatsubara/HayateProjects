---
status: accepted
---

# 要素種別ごとの UA 既定カーソルを core が供給する（明示 cursor → kind 既定 → default の解決順）

**Date: 2026-06-17**

> 本 ADR は決定のみを記録する（実装は後続）。ADR-0088 は cursor を「要素の**明示**
> `cursor` から解決し `on_pointer_move` で adapter に渡す」決定で、`resolve_cursor`
> （`interaction.rs`）は祖先を辿って明示 `cursor` のみを見て、無ければ `Default` を
> 返す。そのため **text-input / 選択可能テキスト上で自動的に I-beam にならない**。
> 加えて Tsubame の Canvas renderer は `on_pointer_move` が返す `resolved_cursor` を
> `canvas` 要素のカーソルへ反映していない（ADR-0088 の adapter 適用が Canvas 経路で
> 結線されていない）。

## Context

ブラウザは UA スタイルシートで element-kind ごとの既定カーソルを与える
（`<input>` / `<textarea>` → `text`（I-beam）、`<button>` → `pointer`）。Hayate は
明示 `cursor` しか解決しないため、全 text-input にアプリが手動で `cursor: text` を
書かない限り I-beam にならず、ブラウザ既定体験とずれる。Semantics Parity 上、この
既定は Canvas / DOM 双方で同一でなければならない。

## Decision

1. **`resolve_cursor` の解決順を「明示 `cursor` → element-kind 既定 → `Default`」に
   する。** kind 既定: **text-input と選択可能テキスト = `text`（I-beam）**、
   **button = `pointer`**、その他 = `default`。ブラウザ UA スタイルシートと同型で、
   要素の明示 `cursor`（ADR-0088 / ADR-0071 の closed vocabulary）が常に優先する。

2. **kind 既定は単一正本（`proto/spec` の element_kinds 由来）から Canvas / DOM 双方が
   参照し、レンダラーごとに再宣言しない**（Semantics Parity）。

3. **カーソル形状は Mouse / Pen modality でのみ意味を持つ**（ADR-0104）。Touch では
   無関係。

4. **Tsubame の Canvas renderer が `resolved_cursor` を実際に適用する**（ADR-0088 の
   「adapter がカーソルを駆動する」を Canvas 経路でも結線する）。

## Considered Options

- **明示 `cursor` のみ（現状維持）** — 全 text-input に手動で `cursor: text` を要求し、
  ブラウザ既定体験とずれる。UA 既定という普遍的期待を満たせない。却下。

## Consequences

- `element_kinds`（または `resolve_cursor` の fallback table）に kind 既定カーソルを
  表現する。
- ADR-0088 の解決パイプラインを**拡張**する（明示と `Default` の間に kind 既定を挿入）。
- Canvas renderer の `resolved_cursor` 未適用バグも併せて解消する。
- 実装は本決定後（本 ADR は決定のみ）。

## 関係

- ADR-0088（pointer から cursor 解決 / `cursor` 語彙）: 本 ADR が kind 既定 fallback を
  解決順に追加し、Canvas 適用を結線。
- ADR-0104（PointerKind）: カーソルは Mouse / Pen modality でのみ表れる。
- ADR-0071（closed vocabulary）: `cursor` 値と element_kinds はその一員。
