# proto/protocol.yaml を Hayate-Tsubame 間プロトコル定数の機械可読な単一正本とする

## Status

Accepted (2026-06-04)

## Context

Hayate と Tsubame の間には `apply_mutations`（Float64Array ops + Float32Array styles）と
`poll_events`（Array<Array<any>>）という2種類のバイナリプロトコルがある。
これらのプロトコルを記述する定数（OP_*、TAG_*、EVENT_KIND_* 等）は、
従来 Hayate 側の Rust ソースと Tsubame 側の TypeScript ソースにそれぞれ手書きで存在し、
ドリフト（不一致）のリスクを常に抱えていた。

また、`Hayate/wit/hayate.wit` が「仕様書」として存在していたが、
wasm-bindgen は WIT を読まないため WIT は実際のコード生成に関与しておらず、
宙に浮いた状態だった。

## Decision

1. **WIT を廃止**する。`Hayate/wit/` ディレクトリを削除し、wit-bindgen も導入しない。
   - wasm-bindgen は WIT に依存しないため実害はない。
   - WIT は仕様書として有用だったが、手書き二重定義の根本原因でもあった。

2. **`Hayate/proto/protocol.yaml` を機械可読な単一正本**とする。
   - 8セクション構成（後述）で全定数を網羅する。
   - ファイルはブロック形式のみの固定 YAML（外部パーサー不要）。

3. **Rust コードは `build.rs` が生成する**（`OUT_DIR/protocol.rs`）。
   - `include!(concat!(env!("OUT_DIR"), "/protocol.rs"))` で取り込む。
   - wasm-bindgen 関数群（`event_kind_*()`、`modifier_*()`、`element_kind_*()`）は削除。

4. **TypeScript コードは `scripts/gen-protocol.mjs` が生成する**（`src/protocol.ts`）。
   - `prebuild` / `pretypecheck` フックで自動実行。
   - `opcodes.ts` を置き換え。

## YAML の 8セクション構成

| セクション | 内容 | 生成定数プレフィックス |
|-----------|------|----------------------|
| `opcodes` | apply_mutations の op_kind | OP_* (u32)、OP_SLOTS |
| `style_tags` | style-packet の TAG | TAG_* (u32) |
| `event_kinds` | poll_events の event discriminant | EVENT_KIND_* (f64) |
| `element_kinds` | OP_CREATE の kind_code | ELEMENT_KIND_* (u32) |
| `unset_kinds` | element_unset_style の kind | UNSET_KIND_* (u32) |
| `modifier_keys` | KeyDown の modifiers ビットマスク | MODIFIER_* (u32) |
| `enums` | dimension_unit / display 等のコード値 + font_family プリセット | 名前そのまま |
| `types` | color (4 f32) / dimension (2 f32) の複合型定義 | — |

## 生成物

### Rust (`OUT_DIR/protocol.rs`)

- `pub const OP_*: u32`、`pub const TAG_*: u32`、`pub const EVENT_KIND_*: f64`、
  `pub const ELEMENT_KIND_*: u32`、`pub const UNSET_KIND_*: u32`、`pub const MODIFIER_*: u32`
- `pub const OP_SLOTS: &[usize]` — op_kind 自身を除いたペイロードスロット数
- `pub enum Op` + `pub fn parse_next_op(ops: &[f64], i: usize) -> Result<(Op, usize), &'static str>`
- `pub enum StyleTag` + `pub fn parse_next_style_tag(packed: &[f32], i: usize) -> Result<(StyleTag, usize), &'static str>`
- `pub fn encode_event(ev: &hayate_core::Event) -> js_sys::Array`
- `pub fn encode_events(events: &[hayate_core::Event]) -> js_sys::Array`

### TypeScript (`src/protocol.ts`)

- `export const OP, TAG, EVENT_KIND, ELEMENT_KIND, UNSET_KIND, MODIFIER as const`
- `export const DIMENSION_UNIT, DISPLAY, FLEX_DIRECTION, ALIGN_ITEMS, JUSTIFY_CONTENT, FONT_FAMILY as const`
- `export const OP_SLOTS: readonly number[]`
- `export type EventPayload` — discriminated union（全 event_kinds）
- `export function parseEvent(ev: unknown[]): EventPayload`

## Consequences

- **プラス**: Rust と TypeScript の定数が protocol.yaml から機械生成されるため、ドリフトが構造的に不可能になる。
- **プラス**: `cargo check` と `npm run typecheck` が両方通れば定数の一致が保証される。
- **マイナス**: YAML の変更時は `cargo build` と `npm run generate` の両方が必要。
- **マイナス**: wasm-bindgen 公開関数（`event_kind_*()`等）が消えるため、
  JS 側がこれらを直接呼んでいた場合は移行が必要（現行の Tsubame は使っていない）。

## Supersedes

- ADR-0013: WIT デュアルレイヤー
- ADR-0015: WIT コンパイル戦略
- ADR-0033: Raw Layer WIT 延期
- ADR-0039: apply_mutations エンコーディング（OP_* 定数部分）
