# `poll_events()` は `Array<Array<any>>` を返す（文字列ペイロード対応）

## Context

Hayate の `poll-events()` は WIT 上 `list<event>` を返す。`event` variant には文字列フィールドを含む型がある。

- `text-input-event`: `{ target: element-id, text: string }`
- `composition-event`: `{ target: element-id, text: string }`
- `key-down-event`: `{ target: element-id, key: string, modifiers: modifier-keys }`

wasm-bindgen 実装の `encode_events()` はイベントを `Box<[f64]>` のフラット配列に変換していた。`f64` スライスに文字列を乗せる手段がないため、これらのフィールドは `..` で無視されていた。`TextInput` / `Composition*` / `KeyDown` のペイロードが JS 側に届かず、テキスト入力・IME・キーボードが機能的に壊れていた。

### 修正の選択肢として検討したもの

**A: 文字列サイドチャネル**（`poll_events()` は `Box<[f64]>` を維持 + `take_event_strings()` を追加）
- 2 コール必要。呼び出し順に依存した暗黙の同期があり、JS 側が順番を誤るとサイレントな腐敗が起きる

**B: `js_sys::Array` of sub-arrays（採用）**
- `poll_events()` の返り値を `js_sys::Array` に変更し、各イベントをサブ配列 `[kind, ...fields]` として格納
- 文字列は自然な位置に存在する。JS 側は 1 コールで完結し、同期の問題がない

**C: serde / JSON**
- 可読性は高いが serde 依存を増やす。UI イベントという高頻度パスへの投入は過剰

## Decision

**`poll_events()` の返り値を `js_sys::Array`（サブ配列の配列）に変更する。**

各サブ配列は `[kind: f64, ...fields]` の形式。`kind` は既存の `event_kind_*()` 定数と一致する。文字列フィールドは JS 文字列として自然な位置に入る。

フィールド順序は WIT の record 定義順に従う（`key-down` は `target → key → modifiers`）。

## Considered Options

上記 A / B / C 参照。B を採用した主な理由は以下のとおり。

- JS 消費側（Hayabusa）がまだ存在しないため `Box<[f64]>` への後方互換コストがない
- サイドチャネル方式（A）は暗黙の同期結合を生む
- `js_sys` は既存の依存（`js-sys` workspace crate）であり追加コストなし

## Consequences

- `encode_events()` の戻り値型が `Box<[f64]>` → `js_sys::Array` に変更
- `HayateElementRenderer::poll_events()` および `HayateHtmlElementRenderer::poll_events()` の戻り値型が同様に変更
- `TextInput` / `Composition{Start,Update,End}` / `KeyDown` のペイロード（`text` / `key`）が JS 側に届くようになりバグ修正
- `key-down` のフィールド順が `[12, target, modifiers]`（旧・破損）から `[12, target, key, modifiers]`（WIT 準拠）に変更
- JS 側のイベントループは `Float64Array` のインデックスアクセスから `Array<Array>` の反復処理に変わる
- `docs/TODO.md` の P3-9「イベント f64 エンコードに文字列を持てない」は本 ADR の実装により解決済みとして削除
