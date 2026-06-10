# element_renderer.rs を Canvas/HTML/共有の3ファイルへ機械的分割する

**Status: accepted**

**Date: 2026-06-10**

## Context

`hayate-adapter-web/src/element_renderer.rs` は `HayateElementRenderer`（Canvas）と `HayateElementHtmlRenderer`（HTML Mode、`HtmlNode` 並列ツリーを保持）の両方を含む god-module になっている。ADR-0037（HTML deferred queue）と ADR-0066（adapter 薄化）は決まっているが、分割の単位は未 ADR だった（`docs/architecture-decisions-pending.md` 項目3）。

## Decision

**既存の自然な境界に沿って機械的に3分割する。新しい抽象化は導入しない。**

- `canvas.rs`（~420行）：`HayateElementRenderer`（Canvas Mode）
- `html.rs`（~900行）：`HayateElementHtmlRenderer` ＋ `HtmlNode` 並列ツリー（HTML Mode）
- `shared.rs`（~100行）：両者が共有するコード

## Considered Options

- **共有 `WasmDocumentFacade` の新設**：Canvas/HTML 間で共有可能なロジックを seam として切り出す案。実際の共有部分は `shared.rs` の~100行程度に収まり、新規 trait/抽象化を正当化するほどの重複が無い。却下。
- **HTML Mode の `HtmlNode` 並列ツリーを `ElementTree` に統合**：`HtmlNode` が保持する DOM handle は Platform Adapter 固有の状態であり、`ElementTree`（core）に持ち込むと「core は Platform Adapter を知らない」という境界（ADR-0030/0037）を破る。HTML Mode が DOM ミラーを持つこと自体は ADR-0030/0037 が選んだ設計の**内在コスト**であり、god-module 化（実装の整理不足）とは別の問題。統合は却下し、現状維持。

## Consequences

- god-module が Canvas/HTML/共有の自然な境界で分割され、各ファイルの責務が読み取りやすくなる。
- `HtmlNode` 並列ツリーの重複は意図的に残る（ADR-0030/0037 の設計選択として明記）。将来「なぜ統合しないのか」を問われた際の参照点になる。

## 関係

- `docs/architecture-decisions-pending.md` 項目3を解決。
- ADR-0030/0037：HTML Mode の deferred queue / DOM ミラー設計。
- ADR-0066：adapter 薄化（event-routing の core 移管）。
