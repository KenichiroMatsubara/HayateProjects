# apply_mutations のエンコーディングを Float64Array + Float32Array の 2 引数とする

_origin: Hayate ADR-0039_

Canvas Renderer が毎フレーム Hayate に mutation を渡す `apply_mutations` の JS→WASM 境界エンコーディングを確定する。hot path（1回/frame）であるため転送効率を最優先とし、`apply_mutations(ops: Float64Array, styles: Float32Array)` の 2 引数形式を採用する。

## 採用した設計

```typescript
apply_mutations(ops: Float64Array, styles: Float32Array): Result<(), JsValue>
```

**ops ストリーム**: 固定長レコードの繰り返し。各レコードは `op_kind` から始まり、op 種別ごとの固定 slot 数を消費する。

| op_kind | slots | layout |
|---------|-------|--------|
| OP_APPEND_CHILD      | 3 | `op, parent_id, child_id` |
| OP_INSERT_BEFORE     | 4 | `op, parent_id, child_id, before_id` |
| OP_REMOVE            | 2 | `op, id` |
| OP_SET_ROOT          | 2 | `op, id` |
| OP_SET_STYLE         | 4 | `op, id, style_offset, style_len` |
| OP_SET_TRANSFORM     | 9 | `op, id, has_matrix, m0, m1, m2, m3, m4, m5` |
| OP_SET_SCROLL_OFFSET | 4 | `op, id, x, y` |
| OP_FOCUS             | 2 | `op, id` |
| OP_BLUR              | 2 | `op, id` |

**styles バッファ**: Hayate の `style_packet.rs` の TAG エンコーディング（flat f32 配列）をそのまま使う。`OP_SET_STYLE` の `style_offset` / `style_len` で参照する。

**エラー処理**: 不明な `op_kind` は `Err(JsValue)` を返してそのフレームの残り op を捨てる。

## バッチ外に置くもの

- **`element_create`**: 戻り値（ElementId）が必要なため個別呼び出し。フレーム開始前に ID を取得し、その ID を ops ストリームで参照する
- **文字列 op**（`element_set_text` / `element_set_src` / `element_set_font_family` 等）: signal 変化時のみ発火で頻度が低く、typed array に収まらない

## Considered Options

- `js_sys::Array`（JsValue 配列）単体: 文字列を含めて全 op を一本化できるが typed array より転送コストが高い
- `Float64Array` 単体（文字列 op も含める）: 文字列を f64 ストリームに埋め込む方法がなく成立しない
- `set_transform` を可変長: 節約できるバイト数は 48 バイト/op で誤差レベル。パーサーの分岐が増えるため採用しない
- 不明 op_kind を skip して続行: 固定長が判明しないためストリームがズレ以降の op が全壊する
