# Hayate-Tsubame 間プロトコル定数の機械可読な単一正本とする

**Status: Accepted (amended by [ADR-0053](0053-element-document-runtime-poll-deliveries.md))**

**Date: 2026-06-04**

> **2026-06-06 追記（ADR-0053）:** 正本形式は当初の単体 `protocol.yaml` から `Hayate/proto/spec/*.json` + JSON Schema + npm パッケージ `@torimi/hayate-protocol-spec` へ移行済み。本 ADR の「単一正本」原則は維持し、形式のみ更新する。

## Context

Hayate（Rust + WASM）と Tsubame（TypeScript）は `apply_mutations` / `poll_events` を介して通信する。
通信に使う定数（`OP_*`, `TAG_*`, `EVENT_KIND_*` など）はこれまで Rust と TS にそれぞれ手書きで定義されており、
ドリフトが発生していた。

WIT（WebAssembly Interface Types）ファイル (`Hayate/wit/hayate.wit`) が「仕様書」として存在したが、
wasm-bindgen は WIT を一切読まない。そのため WIT は宙に浮き、Rust の `event_kind_*()` などの
wasm-bindgen 関数群と TS のハードコード値が二重定義になっていた。

wit-bindgen 導入は今後の検討事項として残すが、短期的解決策として機械可読な単一正本を導入するアプローチを選択した。

## Decision

### WIT の廃止

`Hayate/wit/` ディレクトリを削除し、WIT を廃止する。
ADR-0013, 0015, 0033, 0039 を Superseded とする。

### JSON spec の導入（ADR-0053 で protocol.yaml から移行）

Hayate-Tsubame 間契約の正本は **Hayate リポジトリ** `proto/spec/` の JSON 群とする。

8 セクション分割:

- `enums.json`: `dimension_unit`, `display`, `flex_direction`, `align_items`, `justify_content`, `font_family`
- `types.json`: `color` (r/g/b/a × f32), `dimension` (value: f32 + unit: dimension_unit)
- `opcodes.json`: `apply_mutations` 第1引数 (f64)。各 op の name/value/params
- `element_kinds.json`: `OP_CREATE` の kind コード
- `style_tags.json`: `apply_mutations` 第2引数 style-packet (f32)
- `event_kinds.json`: `poll_events` の kind discriminant と params（`wireRole` / `adapterTier` 含む）
- `unset_kinds.json`: `element_unset_style` の kind コード
- `modifier_keys.json`: キー修飾子ビットマスク (1/2/4/8)

`proto/spec/schema/` で JSON Schema 検証。`proto/scripts/validate-spec.mjs` が CI で実行される。

配布: npm パッケージ `@torimi/hayate-protocol-spec`。Tsubame は正本を持たず workspace 依存として取り込む。

型システム:
- プリミティブ: `element_id`, `u32`, `bool`, `f32`, `f64`, `usize`, `string`
- 複合型 (`types.json`): `color`, `dimension`
- 配列: `type` + `count` フィールドで表現
- enum 参照: `enums.json` の name を type に指定

### コード生成

**Scope（[ADR-0055](0055-wire-codec-single-source.md) で拡張）:** 定数に加え **wire codec**（`apply_mutations` の encode/decode）を spec から生成する。semantic 層（`StylePatch` / HayateRenderer 所有 semantic queue）は Contract 外のまま。

**Hayate: `proto/generator/`（Rust）→ `proto/generated/`（commit 済み）**
- `OP_*: u32`, `TAG_*: u32`, `EVENT_KIND_*: f64`, `ELEMENT_KIND_*: u32`, `UNSET_KIND_*: u32`, `MODIFIER_*: u32` 定数
- `OP_SLOTS: &[usize]` テーブル
- `Op` enum + `parse_next_op`
- `StyleTag` enum + `parse_next_style_tag`
- `encode_event` / `encode_events`（event_kinds 完全 codegen は ADR-0053 Stream C で継続）
- `codec.rs`: `encode_op`, `encode_style_packet`（decode と対称。ADR-0055）

`build.rs` は生成済み `proto/generated/` を取り込む薄型 wrapper。CI で `npm run check:proto`（validate → generate → diff）を実行。

**Tsubame: `proto/generator/`（TS）→ `proto/generated/`（commit 済み）**
- wire 定数、`parseEvent`, delivery wire 型、adapter vocabulary（`StylePatch`, `EventKind` 等）
- `codec.ts`: op 別 append、`encodeStylePatch` / `unsetKindsOf`（`style_tags.encodeFrom` 駆動。ADR-0055）
- `@torimi/hayate-protocol-spec` を入力とする

CI で `pnpm run check:proto`（generate → diff）を実行。

### Event enum フィールド名の統一

`hayate-core` の `Event` enum フィールド名を spec の `params[].name` に揃える。
生成コードが spec 名を直接使うため、乖離はコンパイルエラーで検出される。

例: `Event::Click { target, x, y }` → `Event::Click { target_id, x, y }`

## Consequences

### Positive
- Rust と TS の定数ドリフトがコンパイル時に検出可能になった
- JSON spec が唯一の変更箇所であり、両言語へ同期的に伝播する
- JSON Schema による標準バリデーションが可能
- wit-bindgen 不要でビルドチェーンが単純なまま

### Negative
- generator を Hayate（Rust）と Tsubame（TS）の 2 箇所に置く必要がある
- event encoder の完全 spec 駆動化は段階的（decisions-pending Open #1）

## Supersedes

ADR-0013, ADR-0015, ADR-0033, ADR-0039（WIT 関連）

## Amended by

[ADR-0053](0053-element-document-runtime-poll-deliveries.md) — 正本形式を `protocol.yaml` から `proto/spec/*.json` + JSON Schema へ。generator 配置と CI diff check を追加。
