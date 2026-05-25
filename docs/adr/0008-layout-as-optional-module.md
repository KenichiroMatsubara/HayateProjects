---
status: superseded — Hayate への移行により Layout（Taffy）は Element Layer の必須構成要素となり、optional module として分離しない。
---

# Layout Engine を newdom-core から分離し optional module とする

Layout Engine（Taffy）は `newdom-core` の必須機能ではなく、`newdom-layout` crate として分離する。

Layout Engine は最終的に `nd_node_update(id, { x, y, width, height })` を呼ぶ producer の一種であり、NewDOM コアから見れば通常の Mutation と区別がない。newdom-core は「どのように座標が計算されたか」を知る必要がなく、受け取った props をそのまま描くだけでよい。

CSS が DOM に強く結合したことで Web の描画スタックが巨大化した歴史的失敗を NewDOM で繰り返さないため、Layout を明示的に分離する。

## Consequences

- `newdom-layout` は `newdom-core` に依存し、Taffy で計算した結果を `nd_node_update()` で書き込む
- `newdom-core` は Taffy を直接 import しない
- Layout を使わないユーザー（ゲーム HUD・Infinite Canvas 等）は `newdom-layout` を依存に含めなくてよい
