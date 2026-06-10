# Dirty 管理を内部 module ElementEngine に集約し commit_frame を公開する

**Status: accepted**

**Date: 2026-06-10**

## Context

`structure_dirty` / `shape_dirty` / `fonts_dirty` の dirty 集合は `LayoutPass` / `TaffyProjection` に分散しており、「フレームを進める」ための解決ロジック（dirty 解決＋layout settling）を呼び出す public な口が無い。`ElementTree` から「この frame を確定させたい」を表現する手段が無く、テストや host 側が `LayoutPass::run()` 相当の内部詳細に依存しがちになる。

`docs/architecture-decisions-pending.md` の項目1で「ElementTree から DocumentEngine（または DocumentTree + DocumentSession）へ orchestration を抽出する形」が候補 D2 として未 ADR のまま残っていた。

## Decision

**`ElementTree` を唯一の public type のまま変えない**（リネームしない、既存 ~100 メソッドのシグネチャ不変）。dirty 集合の所有を内部 module **`ElementEngine`**（`element/engine.rs`）に集約し、`ElementTree` の private field とする。

- `ElementEngine` が `structure_dirty` / `shape_dirty` / `fonts_dirty` を一箇所で保持する。
- 新規 public メソッド **`ElementTree::commit_frame()`** を追加。スコープは **A**（`LayoutPass::run()` 相当 = dirty 解決＋layout settling のみ）。`render()` / `resolved_elements()` の統合は行わない。
- dirty を「何のミューテーションでマークするか」という**ポリシー**は `tree.rs` の `element_set_*` 群に残す。`ElementEngine` は集合の保持と解決のみを担う。
- interaction 状態機械（ADR-0066）はそのまま `ElementTree` に残置、本決定の対象外。

## Considered Options

- **`ElementEngine` が `ElementTree` を所有/置換する新 public 型**：adapter の保持型を `ElementEngine` に差し替える案。コスト分析の結果、既存 ~100 メソッドの forwarding が必要になり、`ElementTree` という確立された public surface を壊すだけのリターンが無い。却下。
- **`DocumentEngine` への改称**：「Document」という語は本セッションで `Canonical Tree`（旧 Document Tree）にも使われており、衝突する。`Element*` という既存命名規則（`ElementTree`/`ElementId`/`Element`）に揃え `ElementEngine` を採用。
- **dirty 解決ロジックを `tree.rs` 直書きのまま放置**：dirty 集合が `LayoutPass`/`TaffyProjection` に分散したまま、「フレーム確定」という概念が public API に存在しない状態が継続。却下。

## Consequences

- `ElementTree` の public surface は不変（破壊的変更なし）。
- dirty 集合の所在が一箇所（`ElementEngine`）に集約され、`structure_dirty`/`shape_dirty`/`fonts_dirty` の整合性を1モジュールでレビュー可能。
- `commit_frame()` により host/test が「フレームを確定する」操作を明示的に呼べる（`LayoutPass::run()` の内部詳細に依存しない）。
- dirty marking policy（何が dirty になるか）と dirty resolution（dirty をどう解決するか）の責務分離が明確化。

## 関係

- `docs/architecture-decisions-pending.md` 項目1（候補 D2）を解決。
- ADR-0066：interaction 状態機械の `ElementTree` 内配置は不変。
- ADR-0064：`TaffyProjection` の dirty-scoped reconcile は `ElementEngine` の `structure_dirty`/`shape_dirty` 解決経路に統合される。
