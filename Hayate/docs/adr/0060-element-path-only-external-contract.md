# 外部契約は Element path のみとし、Raw Layer の独立公開は行わない

**Status: accepted**

**Date: 2026-06-07**

**Supersedes:** [ADR-0013](0013-wit-dual-layer.md)（外部二層 export の部分）、[ADR-0033](0033-raw-layer-wit-deferred.md)（WIT export 待ちの前提）

## Context

ADR-0013 は Element Layer と Raw Layer を **ともに WIT で外部公開**し、下位を独立操作できる逃げ道とした。ADR-0033 で Raw Layer の export は一時停止、ADR-0049 で WIT 自体を廃止し、現行の Hayate–Tsubame 契約は `@hayate/protocol-spec` と `apply_mutations` / `poll_events` の Element path のみになった。

開発優先は Tsubame 経由の Element Layer（ADR-0051）。内部では Element → `SceneGraph` lowering は継続するが、ゲーム HUD 等が Raw Layer API を直接触る前提は契約にも実装優先にも含まれていない。

## Decision

**Hayate の現行外部契約は Element path のみとする。** Raw Layer（`SceneGraph` / `Node` 直接 mutation）を host が独立操作する API は公開しない。

- 内部実装としての `SceneGraph`・`scene_build` lowering は維持する
- 将来、下位 API が必要になった場合は **別 ADR** で契約・優先度を決めてから追加する
- ADR-0013 の「二層とも外部公開」という consequence は本 ADR により棄却する

## Considered Options

- **ADR-0013 の二層外部公開を温存（WIT 復活または proto に raw opcode 追加）** — 契約と実装の二重メンテが続き、Tsubame first と矛盾
- **内部 SceneGraph ごと削除** — Element lowering の実装基盤が失われ、過剰
- **Element path のみ外部契約（採用）** — 単一 seam の深さと locality が得られる

## Consequences

- `CONTEXT.md` の Raw Layer は「内部 lowering 先」として記述し、現行公開 API ではないことを明示する
- proto spec に `OP_CREATE_RECT` 等の raw opcode は追加しない（本決定の範囲内）
- Infinite Canvas / ゲーム HUD 向け逃げ道は backlog とし、Element Document Runtime 深掘りを先に進める
