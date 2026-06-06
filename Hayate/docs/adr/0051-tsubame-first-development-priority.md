# Tsubame First Development Priority

**Status: Accepted**

**Date: 2026-06-04**

## Context

Hayate には Hayabusa と Tsubame という二つの文脈がある。Hayabusa は ADR-0045 により Rust crate 依存の長期構想として整理された。一方で短期的に契約整備が必要なのは、Hayate-Tsubame 間の `@hayate/protocol-spec`・`apply_mutations`・`poll_events` である。文書上は依然として Hayabusa 中心、WIT 中心の説明が多く、現行の実装優先度を誤解しやすい。

## Decision

- 現時点の開発優先は Tsubame 側とする。
- Hayate の現行仕様文書は、まず Hayate-Tsubame 間契約を短く確認できる形で整備する。
- Hayabusa は削除しないが、現役の短期優先開発対象としては扱わない。
- `CONTEXT.md` は glossary に戻し、優先度判断や過去案の説明を抱え込まない。

## Consequences

### Positive

- 現行の実装契約と文書の重心が一致する。
- `@hayate/protocol-spec` 正本化以降の説明が読みやすくなる。
- Hayabusa 構想を残しつつ、短期タスクの判断を迷いにくくできる。

### Negative

- Hayabusa 中心で読んでいた既存文書とは重心が変わる。
- 旧 spec や README に残る説明とのズレを段階的に整理する必要がある。
