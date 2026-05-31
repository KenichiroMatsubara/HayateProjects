import type { ElementKind, EventKind } from '@tsubame/renderer-protocol';

/**
 * `apply_mutations` の ops ストリーム opcode。
 *
 * Rust 側の `element_renderer.rs` の OP_* 定数と 1:1 対応。
 * CREATE(9) は JS 側が採番した ElementId とともに WASM へ通知し、
 * Rust 側が ElementTree に登録する（JS→WASM のラウンドトリップ不要）。
 */
export const OP = {
  APPEND_CHILD:    0, // op, parent_id, child_id
  INSERT_BEFORE:   1, // op, parent_id, child_id, before_id
  REMOVE:          2, // op, id
  SET_ROOT:        3, // op, id
  SET_STYLE:       4, // op, id, style_offset, style_len
  SET_TRANSFORM:   5, // op, id, has_matrix, m0..m5
  SET_SCROLL_OFFSET: 6, // op, id, x, y
  FOCUS:           7, // op, id
  BLUR:            8, // op, id
  CREATE:          9, // op, id, kind_code
} as const;

/** opcode ごとの固定 slot 数（opcode 自身を含む）。 */
export const OP_SLOTS: Record<number, number> = {
  [OP.CREATE]:           3,
  [OP.APPEND_CHILD]:     3,
  [OP.INSERT_BEFORE]:    4,
  [OP.REMOVE]:           2,
  [OP.SET_ROOT]:         2,
  [OP.SET_STYLE]:        4,
  [OP.SET_TRANSFORM]:    9,
  [OP.SET_SCROLL_OFFSET]: 4,
  [OP.FOCUS]:            2,
  [OP.BLUR]:             2,
};

/** Element 語彙 → OP_CREATE で送る kind_code（Rust `element_kind_*()` と一致）。 */
export const KIND_CODE: Record<ElementKind, number> = {
  view: 0,
  text: 1,
  image: 2,
  button: 3,
  'text-input': 4,
  'scroll-view': 5,
};

/**
 * `poll_events()` が返すフラット Float64 配列の kind_code → {@link EventKind}。
 *
 * Rust 側 `encode_events_flat` の出力コードと対応:
 *   click=0, focus=1, blur=2, hover-enter=10, hover-leave=11
 */
export const EVENT_KIND_BY_CODE: Record<number, EventKind> = {
  0:  'click',
  1:  'focus',
  2:  'blur',
  10: 'hover-enter',
  11: 'hover-leave',
};

/** poll_events の 1 レコードあたりの slot 数（[kind_code, element_id] の対）。 */
export const EVENT_RECORD_SLOTS = 2;
