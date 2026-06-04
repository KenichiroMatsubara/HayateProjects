# Hayate/proto/protocol.yaml を Hayate-Tsubame 間プロトコル定数の機械可読な単一正本とする

**Status: Accepted**

**Date: 2026-06-04**

## Context

Hayate（Rust + WASM）と Tsubame（TypeScript）は `apply_mutations` / `poll_events` を介して通信する。
通信に使う定数（`OP_*`, `TAG_*`, `EVENT_KIND_*` など）はこれまで Rust と TS にそれぞれ手書きで定義されており、
ドリフトが発生していた。

WIT（WebAssembly Interface Types）ファイル (`Hayate/wit/hayate.wit`) が「仕様書」として存在したが、
wasm-bindgen は WIT を一切読まない。そのため WIT は宙に浮き、Rust の `event_kind_*()` などの
wasm-bindgen 関数群と TS のハードコード値が二重定義になっていた。

wit-bindgen 導入は今後の検討事項として残すが、短期的解決策として protocol.yaml を単一正本とするアプローチを選択した。

## Decision

### WIT の廃止

`Hayate/wit/` ディレクトリを削除し、WIT を廃止する。
ADR-0013, 0015, 0033, 0039 を Superseded とする。

### protocol.yaml の導入

`Hayate/proto/protocol.yaml` をプロトコル定数の機械可読な単一正本とする。

8セクション構成:

- `enums`: `dimension_unit`, `display`, `flex_direction`, `align_items`, `justify_content`, `font_family`
- `types`: `color` (r/g/b/a × f32), `dimension` (value: f32 + unit: dimension_unit)
- `opcodes`: `apply_mutations` 第1引数 (f64)。各 op の name/value/params を定義
- `element_kinds`: `OP_CREATE` の kind コード
- `style_tags`: `apply_mutations` 第2引数 style-packet (f32)。各 tag の name/value/params を定義
- `event_kinds`: `poll_events` の kind discriminant。各イベントの params (string 型含む) を定義
- `unset_kinds`: `element_unset_style` の kind コード
- `modifier_keys`: キー修飾子ビットマスク (1/2/4/8)

型システム:
- プリミティブ: `element_id`, `u32`, `bool`, `f32`, `f64`, `usize`, `string`
- 複合型 (`types:` セクション): `color`, `dimension`
- 配列: `type` + `count` フィールドで表現
- enum 参照: `enums:` セクションの name を type に指定

### コード生成

**Rust: `Hayate/crates/adapters/web/build.rs` → `OUT_DIR/protocol.rs`**
- `OP_*: u32`, `TAG_*: u32`, `EVENT_KIND_*: f64`, `ELEMENT_KIND_*: u32`, `UNSET_KIND_*: u32`, `MODIFIER_*: u32` 定数
- `OP_SLOTS: &[usize]` テーブル（op_kind 自身を除いた payload スロット数）
- `Op` enum + `parse_next_op(ops: &[f64], i: usize) -> Result<(Op, usize), &str>`
- `StyleTag` enum + `parse_next_style_tag(packed: &[f32], i: usize) -> Result<(StyleTag, usize), &str>`
- `encode_event(ev: &Event) -> js_sys::Array` + `encode_events(events: &[Event]) -> js_sys::Array`

`element_renderer.rs` は `include!(concat!(env!("OUT_DIR"), "/protocol.rs"));` で生成コードを取り込む。
`apply_mutations` の match 文は手書きのまま維持する（dispatch の codegen は行わない）。

**TypeScript: `scripts/gen-protocol.mjs` → `src/protocol.ts`**
- `export const OP`, `TAG`, `EVENT_KIND`, `ELEMENT_KIND`, `UNSET_KIND`, `MODIFIER` as const
- `export const FONT_FAMILY`, `DIMENSION_UNIT`, `DISPLAY`, `FLEX_DIRECTION`, `ALIGN_ITEMS`, `JUSTIFY_CONTENT` as const
- `export const OP_SLOTS: readonly number[]`
- `export const UNIT_CODE = DIMENSION_UNIT` (後方互換エイリアス)
- `export type EventPayload` discriminated union
- `export function parseEvent(ev: unknown[]): EventPayload`

`prebuild` / `pretypecheck` フックで `npm run generate` を自動実行する。

### Event enum フィールド名の統一

`hayate-core` の `Event` enum フィールド名を protocol.yaml の `params[].name` に揃える。
生成コードが YAML 名を直接使うため、乖離はコンパイルエラーで検出される。

例: `Event::Click { target, x, y }` → `Event::Click { target_id, x, y }`

## Consequences

### Positive
- Rust と TS の定数ドリフトがコンパイル時に検出可能になった
- protocol.yaml が唯一の変更箇所であり、両言語へ同期的に伝播する
- wit-bindgen 不要でビルドチェーンが単純なまま

### Negative
- YAML パーサーを build.rs と gen-protocol.mjs の両方に手書きしており、フォーマット変更時は両方修正が必要
- protocol.yaml の型システムは独自仕様であり、JSON Schema 等の標準バリデーションがない

## Supersedes

ADR-0013, ADR-0015, ADR-0033, ADR-0039（WIT 関連）
