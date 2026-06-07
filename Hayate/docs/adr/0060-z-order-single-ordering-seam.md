# Z-Order の順序解決を単一 seam（ordered_children）に集約する

**Status: accepted**

**Date: 2026-06-07**

## Context

Z-Order（兄弟間の描画順序）の解決が3箇所に分散し、コンパレータが独立していた。

- `scene_build`（paint）: `sort_by_key(z)` 昇順安定（同 z は document 順）— 2箇所に重複
- `tree.rs` hit-test: `b.z.cmp(a.z).then(b.i.cmp(a.i))` 降順
- `tree.rs` `walk_resolved`（resolved/HTML 経路）: 無ソート（document 順）

paint と hit-test は現状偶然一致していたが、独立コンパレータのため将来ドリフトしうる。ADR-0054 が描画 walk を `render_scene_graph` 1箇所に集約したのと同じ「単一の所在」を、Z-Order の**順序**には与えていなかった。

## Decision

Z-Order の順序解決を `ElementTree::ordered_children(id) -> Vec<ElementId>` の 1 メソッドに集約する。**paint order**（z 昇順・同 z は document 順で安定 = 後勝ち）を唯一の正本とする。

- **paint**（`scene_build`）はこれを前方反復する。
- **hit-test** はこれを `.rev()` で消費する。「hit-test = paint の逆順」を構造的に保証し、独立コンパレータの再導入を不能にする。

stable 昇順の `.rev()` は `(z 降順, index 降順)` となり、従来の hit-test コンパレータと完全一致するため挙動は不変。

## 意図的に seam を通さない経路

`walk_resolved`（`resolved_elements` → HTML Mode、将来の AccessKit）は **document order を保ち、この seam を通さない**。

- HTML Mode はブラウザ CSS の `z-index` で stacking する（ADR-0029）。ここで z ソートすると二重 stacking になる。
- アクセシビリティの読み上げ順は paint order ではなく document order が正しい。

`resolved` は `z_index` をフィールドとして emit し、順序解決は consumer（CSS / AT）に委ねる。

## Considered Options

- **2 メソッド（paint_order / hit_order）を別々に公開**: 明示的だが独立実装の余地が残り、ドリフトを構造的に防げない。
- **現状維持**: コンパレータ3分散のまま。paint/hit の一致が無保証。

## Consequences

- `scene_build` の2重 sort と hit-test の独立コンパレータが `ordered_children` 呼び出しに収束。
- 回帰テストで tie-break（後勝ち）と paint/hit 逆順一致を pin する。
- 関連: ADR-0021（Z-Order = 子順序・stacking context なし）、ADR-0054（単一 walk）。
