# Canonical Tree 走査に共有スケルトンを抽出する（DocumentFrameWalker は採用しない）

**Status: accepted**

**Date: 2026-06-10**

## Context

Canonical Tree を走査する処理が3箇所に独立実装されている：

- `scene_build`（paint order、ADR-0060）
- `walk_resolved`（document order、HTML Mode の DOM sync）
- `walk_accessibility`（document order、AccessKit、`accessibility.rs:80-102`）

いずれも「`id` から Taffy node を引き、無ければどうするか」を個別に実装している。`layout_pass.rs::cache_layout`（394-415行）は Taffy node が無い場合（inline text element など IFC 配下）でも **自身を skip しつつ子は再帰する**正しいパターンを持つが、`walk_accessibility` はこの skip-but-recurse を実装しておらず、Taffy node が無い時点で `return` ＝ **IFC 配下の subtree が AccessKit tree から丸ごと欠落するバグ**になっている。

`docs/architecture-decisions-pending.md` 項目2は、この3走査を `DocumentFrameWalker` ＋ visitor（`SceneGraphEmitter`/`ResolvedElementCollector`）に統一するかを未決としていた。

## Decision

**3走査を統一する `DocumentFrameWalker` は採用しない。** 代わりに、`cache_layout` が持つ skip-but-recurse パターンを共有スケルトン関数として抽出し、3走査がこれを呼ぶ。

- スケルトン: `id` から `TaffyProjection` 経由で taffy node を引く → 無ければ自身を skip し子を再帰 → あれば `(id, taffy_node, &Element)` を caller に yield。
- 子の走査順序（paint order vs document order）は **caller 側が制御**したまま変えない。`walk_accessibility` の document order も維持。
- `InheritedVisualContext`（ADR-0065 の継承 context 計算）はスケルトンの外に残す。`scene_build`/`walk_resolved`/`InlineText` のみが必要とし、`walk_accessibility` は不要なため。

この変更で `walk_accessibility` の IFC subtree 欠落バグも同時に修正される。

## Considered Options

- **`DocumentFrameWalker` + visitor 完全統一**：3走査を1 traversal + visitor に統合。HTML Mode の document-order 要件と Canvas paint-order の差を visitor 層に押し込む必要があり、ADR-0067 が「3 caller・1 resolver」で達成した locality とのバランスを崩す大きめの抽象化。バグ修正に対して過剰。却下。
- **`walk_accessibility` のみ個別修正**：3走査の重複実装は残り、将来また同種のバグが生まれうる。共有スケルトン抽出のほうが低コストで再発を防げるため不採用。
- **IFC orchestration 用の新 module（`IfcEngine`）**：`docs/architecture-decisions-pending.md` 項目4で検討。`is_ifc_root`/`shape`/`byte_index_at_point` 等は既に `inline_text.rs` に自然な置き場があり、`shape_dirty` は ADR-0075 の `ElementEngine` でカバーされる。新規 module は不要と判断し、`tree.rs::resolve_ifc_inline_hit()`（~20行）のみ `inline_text.rs` へ移設。

## Consequences

- `walk_accessibility` の IFC subtree 欠落バグが解消（AccessKit tree が IFC 配下の inline text element を含むようになる）。
- 3走査の「Taffy node 有無の扱い」が1関数に集約され、再発防止になる。
- 走査順序・`InheritedVisualContext` 計算は caller ごとに残り、ADR-0067 が確立した「セマンティクスは1箇所、context 取得経路は複数」という構図を踏襲する。

## 関係

- `docs/architecture-decisions-pending.md` 項目2・項目4を解決。
- ADR-0067：effective style resolver の単一 seam（本決定の前例）。
- ADR-0063/0064/0065：IFC のセマンティクス・dirty・継承。
