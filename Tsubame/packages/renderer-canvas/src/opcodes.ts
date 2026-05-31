import type { ElementKind, EventKind } from '@tsubame/renderer-protocol';

/**
 * `apply_mutations` の ops ストリーム opcode（ADR-0003 / 仕様 §3.3）。
 *
 * ADR-0003 の表に加え、ADR-0005 の決定（ElementId を JS 側採番し createElement も
 * バッチに乗せる）に基づき `OP_CREATE` を追加する。各レコードは opcode から始まり
 * 固定 slot 数を消費する。Hayate 側のパーサーとこの定義が結合点の契約となる。
 */
export const OP = {
  CREATE: 1, // op, id, kind_code
  APPEND_CHILD: 2, // op, parent_id, child_id
  INSERT_BEFORE: 3, // op, parent_id, child_id, before_id
  REMOVE: 4, // op, id
  SET_ROOT: 5, // op, id
  SET_STYLE: 6, // op, id, style_offset, style_len
  SET_TRANSFORM: 7, // op, id, has_matrix, m0..m5
  SET_SCROLL_OFFSET: 8, // op, id, x, y
  FOCUS: 9, // op, id
  BLUR: 10, // op, id
} as const;

/** opcode ごとの固定 slot 数（opcode 自身を含む）。 */
export const OP_SLOTS: Record<number, number> = {
  [OP.CREATE]: 3,
  [OP.APPEND_CHILD]: 3,
  [OP.INSERT_BEFORE]: 4,
  [OP.REMOVE]: 2,
  [OP.SET_ROOT]: 2,
  [OP.SET_STYLE]: 4,
  [OP.SET_TRANSFORM]: 9,
  [OP.SET_SCROLL_OFFSET]: 4,
  [OP.FOCUS]: 2,
  [OP.BLUR]: 2,
};

/** Element 語彙 → OP_CREATE で送る kind_code。 */
export const KIND_CODE: Record<ElementKind, number> = {
  view: 0,
  text: 1,
  image: 2,
  button: 3,
  'text-input': 4,
  'scroll-view': 5,
};

/**
 * `poll_events()` が返す event record の kind_code → {@link EventKind}。
 * event record は `[kind_code, element_id]` の繰り返し（flat Float64Array）。
 */
export const EVENT_KIND_BY_CODE: Record<number, EventKind> = {
  0: 'click',
  1: 'hover-enter',
  2: 'hover-leave',
  3: 'focus',
  4: 'blur',
};

/** poll_events の 1 レコードあたりの slot 数。 */
export const EVENT_RECORD_SLOTS = 2;
