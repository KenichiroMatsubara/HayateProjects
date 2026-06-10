# Canvas Mode クロスシーム統合テストは構造化スナップショット（golden frame）で行う

**Status: accepted**

**Date: 2026-06-10**

## Context

Canvas Mode のテストは3層に分断されている：

- `wasm-integration.test.ts`（C3、ADR-0055）：実 WASM Hayate + `CanvasRenderer`。Mutation Packet の op 順序を assert。`tsubame-solid` は介在しない。
- `solid/src/renderer.test.ts`：`tsubame-solid`（Shadow Tree reconcile）+ stub `IRenderer`。実 WASM は介在しない。
- `edit-context-sync.test.ts`：`syncEditContextBounds` 単体、stub `EditContext`/`RawHayate`。

Shadow Tree（ADR-0062）、Mutation Packet、IME bridge（ADR-0069）を貫通する経路（`tsubame-solid` の reconcile → Mutation Packet → 実 WASM `ElementTree` → IME bounds）が end-to-end で一度も通っていない（`docs/architecture-decisions-pending.md` 項目6）。

## Decision

**「golden frame」をピクセル画像ではなく、ある時点の document state の JSON シリアライズ可能な構造化スナップショットと定義する。**

スナップショットの内容:

- element tree 構造
- `element_effective_visual`（ADR-0067）による effective style
- layout rect（`layout_cache`）
- `accessibility_update()` による AccessKit tree
- IME character bounds（`syncEditContextBounds` 経由）

**mount スコープ**: 既存の C3 fixture（`createNullHayate`）を拡張し、`solid/src/renderer.test.ts` の stub `IRenderer` の代わりに実 `CanvasRenderer`（実 WASM 上）へ `tsubame-solid` を mount する。IME は既存の `EditContext` stub を再利用する。

**前提タスク**: `element_effective_visual` は現在 `ElementTree` の public メソッドだが wasm bindings（`adapters/web/src/element_renderer.rs`）から export されていない。golden frame 取得のため export を追加する。

## Considered Options

- **ピクセル/画像の golden snapshot**：Hayate は GPU 描画（wgpu/Canvas）であり、headless WASM + vitest での画像比較は GPU 依存・brittle。却下。
- **3層を個別に維持**：reconcile op 順序・IME bounds・a11y tree を貫通するクロスシーム回帰が引き続き検出不能なため不採用。

## Consequences

- `tsubame-solid` の reconcile op 順序、Mutation Packet 経由の `ElementTree` 状態、IME character bounds、AccessKit tree を一つのハーネスで golden file 比較できるようになる。
- GPU レンダリングに依存しないため CI で安定して実行できる。
- `element_effective_visual` の wasm export が新規に必要（前提タスク）。

## 関係

- `docs/architecture-decisions-pending.md` 項目6を解決。
- ADR-0062：tsubame-solid の構造専用 Shadow Tree。
- ADR-0067：`element_effective_visual` query（本決定の golden frame の核）。
- ADR-0069：IME bridge（character bounds）。
- ADR-0055：C3 codec 統合テストの前例（`createNullHayate` fixture）。
