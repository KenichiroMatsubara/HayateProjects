# apply_mutations に string table を追加して文字列 op を統合する

**Status: accepted**

ADR-0039 は「文字列 op は typed array に収まらないため `apply_mutations` の外で個別呼び出しする」と決定した。しかしこれは WASM 境界の型制約への補償であり、TypeScript 側の frame transaction が `element_set_text` / `element_unset_style` と `apply_mutations` の呼び出し順序を管理するという責務漏れを生む。コーディングエディタ相当（〜10,000文字）の文字列でも TextEncoder コストは〜300μs で、5フレーム（〜83ms）の許容遅延に対して100倍以上の余裕がある。したがって protocol.yaml を改定して `apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[])` に signature を拡張し、ops ストリームに `OP_SET_TEXT(op, id, text_index)` と `OP_UNSET_STYLE(op, id, kind)` を追加することで、順序管理を Rust 側に完全に移譲する。TypeScript は ops と texts を組み立てて一括送信するだけでよく、呼び出し順序の知識が不要になる。

## Considered Options

- **現状維持（TypeScript が順序管理）**: HayateRenderer が `drainTypedBatch` → `element_set_text` の順序を保証する。動作するが、順序ポリシーが TypeScript に残る。
- **専用経路（apply_string_mutations を別関数）**: TypeScript はまだ「typed batch を先に呼んでから string batch を呼ぶ」順序を知る必要があり、問題を解決しない。

## Consequences

- `protocol.yaml` の `apply_mutations` シグネチャ変更が必要（破壊的変更）。
- Rust 側は ops ストリームを先頭から処理するだけで順序が自然に確定する。
- HayateRenderer の `drainTypedBatch` / flush-on-enqueue ロジックは不要になる。
