# 未決定設計事項

このセッション（2026-06-04）で議論したが実装前にまだ確定が必要な項目。

---

## 1. protocol.yaml への追加候補（要確認）

`Hayate/proto/protocol.yaml` の `style_tags` / `opcodes` と同様に
契約ファイルに入れるべきか未確認の定数群。

### 1-A. イベント種別コード（最優先）

**問題:** `canvas-renderer.ts` の switch 文がマジックナンバーを直書きしており、
Rust の `event_kind_*()` wasm-bindgen 関数群との二重定義ドリフトが起きている。
これが引き継ぎコンテキストで「event variant が food い違っていた」と言われていた問題の実体。

**Rust 定義:** `element_renderer.rs:236-295`
```
click=0, focus=1, blur=2, text_input=3,
composition_start=4, composition_update=5, composition_end=6,
scroll=7, resize=8, active_end=9,
hover_enter=10, hover_leave=11, key_down=12,
active_start=13, pointer_move=14
```

**TS 定義:** `canvas-renderer.ts:182-206` — ハードコードの switch case

**候補セクション:**
```yaml
event_kinds:
  - { name: click,               value: 0 }
  - { name: focus,               value: 1 }
  - { name: blur,                value: 2 }
  - { name: text_input,          value: 3 }
  - { name: composition_start,   value: 4 }
  - { name: composition_update,  value: 5 }
  - { name: composition_end,     value: 6 }
  - { name: scroll,              value: 7 }
  - { name: resize,              value: 8 }
  - { name: active_end,          value: 9 }
  - { name: hover_enter,         value: 10 }
  - { name: hover_leave,         value: 11 }
  - { name: key_down,            value: 12 }
  - { name: active_start,        value: 13 }
  - { name: pointer_move,        value: 14 }
```

また各イベントのフィールド構造（`[kind, target, x, y]` 等）も
style_tags の params 同様に定義できる。poll_events の返り値フォーマット（ADR-0034）が正本。

**未決定:** protocol.yaml に追加するか？ fields 構造も定義するか？

---

### 1-B. UNSET_KIND（継承スタイルリセット種別）

**問題:** `element_unset_style` の kind コードが TS と Rust で二重定義。

**Rust 定義:** `element_renderer.rs` 内（`element_unset_style` 関数）
**TS 定義:** `style-encoder.ts:63-68`
```typescript
export const UNSET_KIND = {
  color: 0, fontSize: 1, fontFamily: 2, fontWeight: 3,
} as const;
```

**候補セクション:**
```yaml
unset_kinds:
  - { name: color,       value: 0 }
  - { name: font_size,   value: 1 }
  - { name: font_family, value: 2 }
  - { name: font_weight, value: 3 }
```

**未決定:** protocol.yaml に追加するか？（ADR-0047 参照）

---

### 1-C. 修飾キービットマスク

**問題:** `modifier_shift/ctrl/alt/meta` が Rust の wasm-bindgen 関数として公開されているが
TS 側での参照方法が不明確。

**Rust 定義:** `element_renderer.rs:300-315`
```
shift=1, ctrl=2, alt=4, meta=8
```

**候補セクション:**
```yaml
modifier_keys:
  - { name: shift, value: 1 }
  - { name: ctrl,  value: 2 }
  - { name: alt,   value: 4 }
  - { name: meta,  value: 8 }
```

**未決定:** protocol.yaml に追加するか？ bitmask であることをどう表現するか？

---

## 2. 動的フォント追加（font_family enum との接続）

**問題:** `hayate.config.json` は `{ "family": "Inter", "url": "..." }` と文字列で登録するが、
`protocol.yaml` の `font_family` enum は整数 ID を使う。この2つの接続が未設計。

**現状:** `protocol.yaml` の `font_family` enum はプリセット（sans_serif/serif/monospace/system_ui 等）のみ。
ユーザー追加フォントの ID 割り当ては未決定。

**検討すべき設計:**
- `hayate.config.json` に `family_id: 100` フィールドを追加して明示的に ID 指定
- あるいは登録順に ID を自動採番（起動時に何番から始まるかを規約化）
- `font_family` enum の予約範囲（0-99 = プリセット、100+ = ユーザー定義 等）

**参照:** ADR-0044（hayate.config.json）、ADR-0043（FetchFont dispatch）

---

## 3. 今セッションで確定した設計（実装待ち）

実装は別セッションで行う。ブランチ: `claude/hayate-candidate-5-wit-y4Vwn`

### 3-1. protocol.yaml 構造（確定済み）

```
Hayate/proto/protocol.yaml
├── enums:
│   ├── dimension_unit        (px/percent/auto/fr)
│   ├── display_value         (flex/grid/block/none)
│   ├── flex_direction_value
│   ├── align_value
│   ├── justify_value
│   └── font_family           (sans_serif/serif/monospace/system_ui)
├── types:
│   ├── color                 (r/g/b/a × f32)
│   └── dimension             (value: f32 + unit: dimension_unit)
├── opcodes:                  (apply_mutations 第1引数 f64、params/type/count付き)
├── element_kinds:            (OP_CREATE の kind_code)
├── style_tags:               (apply_mutations 第2引数 f32、params/type付き)
├── event_kinds:              (poll_events の kind discriminant、params/type付き、string型含む)
├── unset_kinds:              (element_unset_style の kind コード)
└── modifier_keys:            (キー修飾子ビットマスク 1/2/4/8)
```

**型システム:**
- プリミティブ: `element_id`, `u32`, `bool`, `f32`, `f64`, `usize`, `string`
- 複合型 (`types:` セクション): `color`, `dimension`
- 配列: `{ type: f64, count: 6 }` 形式
- enum 参照: `enums:` セクションの名前を type に指定

**`string` 型の使用方針:** 「使う必要があるから使う」。不要な場面（font_family）は enum に変えた。
必要な場面（event_kinds の text_input.text、key_down.key）では使う。

### 3-2. コード生成（確定済み）

**Rust (build.rs → OUT_DIR/protocol.rs):**
- `OP_*: u32`、`TAG_*: u32`、`EVENT_KIND_*: f64`、`ELEMENT_KIND_*: u32`、`UNSET_KIND_*: u32`、`MODIFIER_*: u32` 定数
- `OP_SLOTS: &[usize]` テーブル（param count から導出）
- `Op` enum + `parse_next_op(ops, i)` 関数
- `StyleTag` enum + `parse_next_style_tag(packed, i)` 関数
- `encode_event(ev: &Event) -> js_sys::Array` 関数（encode_events の内部ループを生成）

**TS (scripts/gen-protocol.mjs → src/protocol.ts):**
- `export const OP`, `TAG`, `EVENT_KIND`, `ELEMENT_KIND`, `UNSET_KIND`, `MODIFIER` as const
- `export const FONT_FAMILY`, `DIMENSION_UNIT`, `DISPLAY`, `FLEX_DIRECTION`, `ALIGN_ITEMS`, `JUSTIFY_CONTENT` as const（enum 定数）
- `push*` encoder 関数（全 op・全 style tag）
- `export type EventPayload = ...`（discriminated union）
- `export function parseEvent(ev: unknown[]): EventPayload`

### 3-3. 削除するもの（確定済み）

- `Hayate/wit/` ディレクトリ全体
- `element_kind_*()` wasm-bindgen 関数群（6関数）
- `event_kind_*()` wasm-bindgen 関数群（15関数）
- `modifier_shift/ctrl/alt/meta()` wasm-bindgen 関数群（4関数）
- ADR-0013, 0015, 0033, 0039 → Superseded にマーク
- `Tsubame/packages/renderer-canvas/src/opcodes.ts` → `protocol.ts` に置き換え
- `Tsubame/packages/renderer-canvas/src/style-encoder.ts` の `TAG`/`UNSET_KIND`/`UNIT_CODE`/`DISPLAY_CODE` 等 → `protocol.ts` に移動
- `canvas-renderer.ts` の switch 文（magic number）→ `parseEvent()` を使う形に変更

### 3-4. 新規 ADR（確定済み）

- **ADR-0049:** `Hayate/proto/protocol.yaml` を Hayate-Tsubame 間プロトコル定数の機械可読な単一正本とする

---

## 4. 未決定として残った事項（次セッションで確認）

### 4-1. event_kinds のフィールド構造の codegen 範囲（確認済み）
- TS: `parseEvent()` 生成 ✅
- Rust: `encode_events` 生成 ✅
- 上記は確定済み。実装時に event_kinds の全 params を protocol.yaml に書き出す必要あり。
  `poll_events` の返り値フォーマットは ADR-0034 を参照して各 event の fields を確認すること。

### 4-2. 動的フォント追加（hayate.config.json との接続）
`protocol.yaml` の `font_family` enum（プリセット）と
`hayate.config.json` のユーザー追加フォント（文字列）の ID 接続が未設計。
- 案: enum 値 0-99 = プリセット、100+ = ユーザー定義（hayate.config.json で family_id を指定）
- ADR として起こす前に設計を詰める必要あり

---

## 5. CONTEXT.md 更新候補

以下の語彙が今セッションで使われたが CONTEXT.md にない可能性がある。

- **protocol.yaml**: Hayate と Tsubame の間のプロトコル契約ファイル
- **wire type**: f64/f32 バッファ上の物理型（element_id, bool, f32, usize 等）
- **semantic type**: wire type の解釈後の意味型（Dimension, Color 等）
