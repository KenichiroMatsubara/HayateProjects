# Solid の text は Hayate text element として Document Tree に載せる

> **Status: leaf-string/collapse 部分を [ADR-0063](0063-text-element-inline-formatting-context.md) が supersede（2026-06-07）。**
> 「text は常に Hayate `text` element（正の `ElementId`・仮想 TextNode なし）」の核は維持。ただし本 ADR の「`text` を単一文字列 leaf として持ち、text-in-text を親へ `setText` 集約（collapse）する」モデルは ADR-0063 が覆した — `text` は inline formatting context となり、子 `text` は collapse でなく inline span（styled range）として合成される。下記 tie-break（DOM `span` 構造）と collapse 前提の記述は ADR-0063 を正とする。

tsubame-solid は Solid `createTextNode` を仮想 `TextNode`（負の ID・Hayate 未登録）として保持し、兄弟 text を結合して親へ `setText` する。Hayate は既に `ElementKind::Text` を tree 子として持つ。仮想 text と親集約は Document Tree の外に状態を置き、`<text>` JSX と runtime が一致しない。Solid の text はすべて Hayate `text` element（正の `ElementId`）として create / append / `setText` する。`button` 直下の文字列も子 `text` element とする（親 `button` へのラベル集約はしない）。

性能が Canvas と DOM で拮抗し、計測で優劣がつかない場合は **DOM Renderer の構造**（`button` > `span` 子ノード + `textContent`）を仕様の tie-break 正本とする。

## Considered Options

**仮想 TextNode + 親 `setText` 集約を維持する案を却下。** Hayate に載らない text 状態が残り、論点2（Document Tree の一本化）が閉じない。

**`button` ラベルのみ `element_set_text(button, …)` に集約する案を却下。** `text` だけ別通道になり、tree モデルが二系統になる。形の一貫性を損なう。

## Consequences

- `tsubame-solid` の `TextNode` / `refreshText` / 負数仮想 ID を廃止
- `createTextNode` → `createElement('text')` + `setText`（対象 element 自身）
- Canvas / DOM / Hayate 共通で **text は常に tree 上の `text` element**
- ADR-0057 の Host 側は `ElementId` ハンドル列のみ（text も element）。Document Tree 正本は Hayate（Canvas）または DOM（DOM Renderer）
- 性能 tie-break 時は DOM 構造を正とする（ADR-0006 の kind→タグ対応と整合）
