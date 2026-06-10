# Architecture decisions pending

2026-06-09 architecture review で洗い出した **ADR 不足**（accepted ADR の実装ギャップではない項目）のメモ。実装タスクは GitHub Issues を参照。

出典: `/tmp/architecture-review-20260609-093827.html`（ローカル生成物。内容は本ファイルに要約済み）

## Open

### 1. Element Document Runtime の core 分解（DocumentEngine）

- **状況:** ADR-0066 は interaction 状態機械の core 移管と `element_renderer.rs` からの event-routing 剥離を決めたが、**ElementTree から DocumentEngine（または DocumentTree + DocumentSession）へ orchestration を抽出する形**は未 ADR。「候補 D2」として ADR-0066 consequences に言及のみ。
- **未決:** `commit_frame()` の責務境界、dirty propagation policy の owner、interaction 状態機械との同居 vs 分離。
- **根拠:** ADR-0066（候補 D2）、ADR-0053

### 2. Canonical Tree 走査の単一化（DocumentFrameWalker）

- **状況:** ADR-0067 は effective style resolver の単一 seam までで止め、**caller ごとに継承 context 取得が違う**ことを許容。`scene_build` と `walk_resolved` の二重走査はスコープ外のまま残存。
- **未決:** 単一 traversal + visitor（SceneGraphEmitter / ResolvedElementCollector）を採用するか、HTML Mode の document-order 要件と Canvas paint-order の差を visitor 層でどう扱うか。
- **根拠:** ADR-0067、ADR-0056、ADR-0028/0029（HTML Mode）

### 3. Platform Adapter モード別モジュール分割

- **状況:** ADR-0037（HTML deferred queue）と ADR-0066（adapter 薄化）は決まっているが、**CanvasDocumentHost / HtmlDocumentHost の分割**は未 ADR。`element_renderer.rs` の god-module 化は継続。
- **未決:** 共有 `WasmDocumentFacade` の seam、HTML Mode の `HtmlNode` 並列ツリーを統合できるか、trait 分割の粒度。
- **根拠:** ADR-0066（候補 D2）、ADR-0037、ADR-0030

### 4. Inline Formatting Context オーケストレーション集約（IfcEngine）

- **状況:** ADR-0063/0064/0065 は IFC 意味論・投影・継承を決めたが、**compose / dirty / hit-test / scene emission のライフサイクルを一 module に束ねる**決定はない。現状 5+ module に orchestration が散在。
- **未決:** `IfcEngine`（または同等）の interface、`ElementTree` からの委譲点。
- **根拠:** ADR-0063、ADR-0064、ADR-0065

### 5. Hayate Protocol Contract から Renderer Protocol 語彙を生成する範囲

- **状況:** Hayate/Tsubame CONTEXT は `setProperty` / `addEventListener` / `resize` を Contract 外と明記（意図的保留）。一方 `StylePatch` / `HayateStyle` / `EventKind` は手書きのまま spec と二重管理。
- **未決:** `style_tags` / `event_kinds` から adapter 向け TS 型を codegen するか、DOM 警告 registry との同期方針。
- **根拠:** ADR-0055、ADR-0070、Hayate `CONTEXT.md`（Hayate Protocol Contract）、Tsubame `CONTEXT.md`

### 6. Canvas Mode クロスシーム統合テスト戦略

- **状況:** Shadow Tree（ADR-0062）、Hayate Mutation Packet、IME bridge（ADR-0069）を跨ぐ **golden-frame ハーネス**の方針が未 ADR。各層は単体テストのみ。
- **未決:** headless WASM + tsubame-solid mount のスコープ、assert 対象（op 順序 / IME bounds / poll_events）。
- **根拠:** ADR-0062、ADR-0069、ADR-0071

## Implementation gaps (tracked as issues)

accepted ADR に対する実装不足は GitHub Issues で追跡する（`ready-for-agent`）:

| ADR | Issue | Scope |
| --- | ----- | ----- |
| ADR-0071 | [#94](https://github.com/KenichiroMatsubara/HayateProjects/issues/94) | Semantic properties through `apply_mutations` |
| ADR-0071 / ADR-0052 | [#95](https://github.com/KenichiroMatsubara/HayateProjects/issues/95) | Pseudo-style through `apply_mutations` |
| ADR-0064 | [#96](https://github.com/KenichiroMatsubara/HayateProjects/issues/96) | Dirty-scoped Taffy Projection reconcile |

## Out of scope

- Hayabusa 実装（長期構想のみ。ADR-0045 / ADR-0051）
- WIT 復活や Contract から IRenderer 実装までの完全 codegen（CONTEXT が明示的に却下）
