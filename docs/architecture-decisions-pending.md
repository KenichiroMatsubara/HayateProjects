# Architecture decisions pending

2026-06-09 architecture review で洗い出した **ADR 不足**（accepted ADR の実装ギャップではない項目）のメモ。実装タスクは GitHub Issues を参照。

出典: `/tmp/architecture-review-20260609-093827.html`（ローカル生成物。内容は本ファイルに要約済み）

## Open

（2026-06-09 architecture review で洗い出した6項目はすべて解決し、ADR-0075〜0079 として記録した。）

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
