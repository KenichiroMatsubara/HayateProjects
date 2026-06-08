# Hayate の公開サーフェスは Element Layer 1つに限定し、Raw Layer の外部公開を棄却する

**Status: accepted（公開 Raw Layer の宙吊り意図を正式棄却。ADR-0013/0033 は ADR-0049 で既に superseded）**

**Date: 2026-06-07**

## Context

ADR-0013 は Hayate の公開 WIT を Element Layer（上位）と Raw Layer（下位）の**二層公開**と定め、Raw Layer を game HUD / Infinite Canvas / カスタム layout engine 向けに公開するとした。ADR-0033 はその Raw Layer WIT を完成まで world export から除外（deferred）。両者は **ADR-0049（WIT 廃止・`proto/spec/*.json` の JSON 契約へ移行）で superseded**。

だが ADR-0049 が消したのは **WIT という機構**であって「Raw Layer を公開する」という**意図**ではない。結果、公開 Raw Layer は**宙吊り**になっている：

- 現状コードは Raw Layer = Rust 内部のみ（§4 REND-12 ⬜）。
- 一方 **CONTEXT.md はまだ「Element Layer と Raw Layer の二層 WIT インターフェース」「Raw Layer … 向けに公開する」と宣伝**（stale）。
- §4 REND-12 は「将来再検討」、§3 LAY-03 は「公開契約は未整備」＝ **TODO 扱い（棄却ではない）**。

## Decision

**Hayate の外部公開サーフェスは Element Layer ベースの proto 契約（`apply_mutations` / `poll_events`、ADR-0049/0053）の一つだけとする。Raw Layer（`SceneGraph` / `Node` 直接）の外部公開契約は持たない＝正式に棄却する。** 「将来再検討」を閉じる。

- Raw Layer は **Element Layer の内部 lowering target** に限定（ADR-0008 の二層分離・`render_scene_graph` 共有は内部実装として維持）。
- 棄却対象は **外部公開契約**（旧 Raw Layer WIT / 第2の proto サーフェス）であって、内部 Rust の lowering 構造ではない。

## Rationale

- 外部公開サーフェスが2つあると **第2 wire 契約**（spec / codegen / 検証 / 互換維持）が二重コスト。Raw Layer 公開には別契約が要る。
- Raw Layer は Element Layer の lowering 先（内部）。その**安定公開契約を維持するコストに見合う需要が現時点で無い**。layout-free 直接 GPU プリミティブの需要（game HUD / Infinite Canvas）は Element Layer の絶対座標 / transform で概ね賄える、または scope 外。
- 単一公開境界は本基盤の「型と経路で言い切る／公開チャネルを増やさない」原則と整合。

## Consequences

- game HUD / Infinite Canvas / カスタム layout engine 向けの「layout-free 直接 GPU プリミティブ**公開**」は提供しない。
- CONTEXT の「二層 WIT インターフェース」「Raw Layer … 向けに公開する」を訂正（Raw Layer は内部実装）。Raw Layer glossary の _Avoid_「内部 API（WIT で外部公開されるため）」を反転（内部 API が正）。
- §4 REND-12 を「非公開で確定（将来再検討を閉じる）」、§3 LAY-03 の「公開契約は未整備」を「外部公開しない（確定）。内部の二層分離は維持」に。
- 「二層」は **内部構造**（Element Layer が Raw Layer に lowering）としては残るが、「二層**公開**インターフェース」ではない。
- 将来 layout-free 公開が真に必要になれば、その時点で本 ADR を supersede して reopen（需要が seam を本物にしてから）。

## Considered Options

- **公開 Raw Layer を維持／将来公開（現状の宙吊り）**：第2契約コスト・需要不明のまま TODO が残る。却下。
- **公開を Element Layer 1つに限定（本決定）**：公開境界を1つに。Raw Layer は内部 lowering target。

## 関係

- ADR-0013（WIT 二層公開）・ADR-0033（Raw Layer deferred）：ADR-0049 で機構 superseded、本 ADR が「公開する意図」を正式棄却。
- ADR-0049：WIT 廃止・proto 契約が単一公開正本。
- ADR-0008 / §3 LAY-03：Element Layer（layout 統合）と Raw Layer（layout 非依存）の**内部**二層分離は維持。本 ADR は**外部公開**のみ棄却。
- ADR-0053：Element Document Runtime が公開モデル。
