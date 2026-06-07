# wire codec の単一正本（encode / decode 対称化）

**Status: accepted**

**Date: 2026-06-07**

> **2026-06-07 追記:** 本 ADR は当初「event 方向（Rust encode / TS decode）は既に対称」と仮定し、検証層 C1–C4 を `apply_mutations` 方向に限定した。しかし「両側が同一 spec から生成される」ことは「両生成器が spec を同一解釈する」保証にはならない（`encodeFrom` は TS のみ、`adapterTier` / `interactionKind` は TS delivery 生成のみが消費する等の非対称が現存）。delivery wire（`poll_events` の `[listener_id, kind, ...fields]`）には両言語を突き合わせる共有 fixture が無く、TS `delivery.test.ts` はハードコード行に依存していた。**この盲点を是正し、delivery 方向にも共有 fixture による検証層（C5、下記）を追加する。** 設計確定・実装は未着手（設計書 §10 PROTO-17）。

## Context

ADR-0049 はプロトコル**定数**の単一正本を導入した。`apply_mutations` については Rust 側 decode（`parse_next_op` / `decode_style_packet`）のみ生成され、TS 側 encode（`hayate-mutation-packet.ts` / `style-codec.ts`）は手書きのまま残っていた。event 方向（Rust encode / TS decode）は既に対称。mutation/style の op セマンティクスが二重管理になっている。

## Decision

### Scope 拡張（ADR-0049 の原則を継承）

単一正本の scope を **wire codec** まで広げる。

- **spec が所有するもの**: wire 形式（`opcodes.json` / `style_tags.json`）と **TS 向け入力変換規則**（`encodeFrom`）
- **spec が所有しないもの**: `StylePatch` / `HayateMutationPacket` の semantic 層（Renderer Protocol。Contract 外）

### Spec: `encodeFrom`（style_tags.json）

各 style tag entry に `encodeFrom` を追加（P1）。初期語彙:

| encodeFrom | 入力（TS） | wire |
|------------|-----------|------|
| `css-color` | CSS 色文字列 | tag + r,g,b,a |
| `dimension` | `HayateDimension` | tag + value + unit |
| `f32` | number | tag + f32 |
| `enum:display` 等 | Renderer Protocol enum | tag + enum code |
| `font-family` | string | tag + len + bytes |
| `z-index` | integer | tag + i32 |

`unsetKindsOf` は `unset_kinds.json` + catalog `patchKey` 対応から生成。

### Generator 出力

**Hayate `proto/generated/`**

| ファイル | 内容 |
|----------|------|
| `protocol.rs` | 定数 + decode（現状維持） |
| `codec.rs`（新規） | `encode_op(buf, &Op)`、`encode_style_packet(buf, &[StyleProp])` |

**Tsubame `proto/generated/`**

| ファイル | 内容 |
|----------|------|
| `protocol.ts` | 定数 + `parseEvent`（現状維持） |
| `codec.ts`（新規） | op 別 `appendCreate` 等（A1）、`encodeStylePatch`、`unsetKindsOf`（T2 完成形。parser 含む） |

手書き `style-codec.ts` / `style-encoder.ts` shim は**削除**。`hayate-mutation-packet.ts` は semantic キューのみ残し flush は generated codec を呼ぶ。

### 対称の定義

- **同一 spec** から Rust decode + Rust encode + TS encode を生成する。
- 本番データ流は TS→Hayate の一方向だが、**Rust encode も生成**する（roundtrip テスト・将来の非 Tsubame Rust クライアント向け）。

### 検証（4 層）

| 層 | 内容 |
|----|------|
| **C4** | `proto/spec/fixtures/` に期待 wire を commit（正本） |
| **C1** | Rust: fixture → encode → decode roundtrip |
| **C2** | TS: fixture → encode 出力を fixture と照合 |
| **C3** | TS flush → WASM `apply_mutations` 結合テスト |
| **C5**（delivery 方向。2026-06-07 追加） | `proto/spec/fixtures/delivery_encode.json`（`[{name, kind, fields, wire}]`、positional、全 event kind）を正本とし、Rust は `event → encode_event → wire` 照合、TS は `wire → parseEvent → kind+fields` 照合。両側が同一 fixture を本番方向で参照し delivery wire の drift を検出する。 |

C3 は WASM ビルドコストが高い場合、CI で wasm 変更時ゲートに分離してよい。

## Out of Scope（本 ADR の実装範囲外）

- Renderer Protocol（`IRenderer` / `StylePatch`）の spec 化
- `HayateMutationPacket` semantic mutation 列の spec 生成

## Implementation Tasks

### Task 1 — spec + schema

1. `style_tags.json` 全 entry に `encodeFrom` を追加。
2. `proto/spec/schema/style_tag.schema.json` に `encodeFrom` enum を定義。
3. `proto/spec/fixtures/` を新設（ops / style-patch 入力→wire 期待値）。

### Task 2 — Hayate generator（Rust）

1. `generate_codec()` を追加し `proto/generated/codec.rs` を出力。
2. `encode_op(buf: &mut Vec<f64>, op: &Op)` — `parse_next_op` と対称（M1）。
3. `encode_style_packet(buf: &mut Vec<f32>, props: &[StyleProp])`。
4. `build.rs` / `check:proto` に codec 生成と diff を含める。

### Task 3 — Tsubame generator（TS）

1. `gen-codec.mjs` を新設。`generate.mjs` から呼ぶ。
2. op 別 typed `appendCreate` / `appendSetStyle` / …（A1）。
3. `encodeStylePatch` / `unsetKindsOf` — `encodeFrom` から parser + dispatch を生成（T2 完成形）。
4. `@tsubame/protocol-generated` exports に `./codec` 追加。

### Task 4 — renderer-canvas 移行

1. `style-codec.ts` / `style-encoder.ts` 削除。
2. `hayate-mutation-packet.ts` flush を generated `codec.ts` 呼び出しに差し替え。
3. `hayate.ts` の `parseColor` 等 — generated codec に吸収されたら dead code を削除（またはテスト用に残すか判断）。

### Task 5 — テスト（C1–C4）

1. **C4**: fixtures を先に書く（generator 実装のターゲット）。
2. **C1**: `Hayate/proto/` または `hayate-core` に roundtrip テスト。
3. **C2**: `Tsubame/proto/` または `renderer-canvas` に golden テスト。
4. **C3**: 既存 wasm / renderer-canvas 結合テストを generated codec 経由に更新。

### Task 6 — CI

1. `npm run check:proto` / `pnpm run check:proto` に codec.ts / codec.rs diff を含める。
2. C3 用 wasm ジョブ（全体 PR または wasm 変更時）。
