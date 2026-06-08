# 文書ツリーは backend ごとに一つだけ保持する

> **Status: Superseded by [ADR-0062](0062-tsubame-solid-structure-only-shadow-tree.md)（2026-06-07）。**
> 本 ADR の核「描画・layout・hit-test の正本は単一（Canvas: Hayate `ElementTree` / DOM: ブラウザ DOM）」は ADR-0062 が継承する。一方、「`tsubame-solid` の `TsubameNode` から構造ミラーを撤去し JS 側に構造を一切持たない」という結論は ADR-0062 が**覆した** — solid-js/universal が VDOM を持たず同期で読めるホストツリーを要求する以上、batch 境界の向こうに正本を置く Tsubame では近側に構造専用 reconcile index が不可避だから。詳細・コスト分析は ADR-0062。下記 Consequences のうち「`TsubameNode` 構造ミラー撤去」は無効、それ以外（renderer parent map 撤去・仮想 TextNode 廃止・subtree 片付けは backend）は有効。

Canvas 経路では Hayate `ElementTree`、Tsubame `CanvasRenderer` の parent map、`tsubame-solid` の `TsubameNode` shadow tree が同じ document 構造を並行管理している。`solid-js/universal` がホストツリー走査（`getParentNode` 等）を要求するため Adapter 側に shadow を置いたが、構造の正本が JS に複製され、Solid の fine-grained mutation モデルと二重化している。実質 React の VDOM 上に Solid を乗せた形であり、Signal の変化はツリー差分ではなく `IRenderer` mutation として backend へ届くべきである。文書構造の正本は backend 一箇所に限定する。

## Considered Options

**shadow tree + renderer map + Hayate の三重管理を維持する案を却下。** ADR-0053 で bubble 用 JS map 撤去を決めたが shadow が残り、構造の三重管理が継続している。

**Adapter 側で独自の document tree を正本にする案を却下。** Hayate Document Runtime（ADR-0053）と役割が衝突し、Canvas/HTML のセマンティクス共有が崩れる。

**Adapter は `ElementId` ハンドルのみ保持し、ツリー走査は backend に委譲する案を採用。** Canvas/HTML は Hayate `ElementTree`、Tsubame DOM Renderer はブラウザ DOM が正本。Adapter の `getParentNode` 等は正本への問い合わせ thin wrapper に留める。

## Consequences

- `CanvasRenderer` / `DomRenderer` の `parentOf` / `childrenOf` を撤去（ADR-0053 consequence の完了）
- `tsubame-solid` の `TsubameNode` から構造ミラー（`parent` / `children`）を撤去し、`ElementId` ハンドル化
- text は ADR-0058 に従い Hayate `text` element として Document Tree に載せる（仮想 TextNode 廃止）
- `removeChild` の subtree 片付けは backend が担う
- React Fiber / Vue VDOM は各ランタイムの reconciler として別論点。Tsubame が第二の document tree を持たないことが本 ADR の境界
