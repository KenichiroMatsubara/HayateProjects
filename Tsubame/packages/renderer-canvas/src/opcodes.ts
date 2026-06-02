import type { ElementKind } from '@tsubame/renderer-protocol';

/**
 * `apply_mutations` の op_kind コード（Hayate ADR-0039）。
 *
 * Hayate `crates/adapters/web/src/element_renderer.rs` の `OP_*` 定数と
 * 1:1 で一致させること。ops ストリームは固定長レコードの繰り返しで、
 * 各レコードは op_kind から始まり種別ごとの固定 slot 数を消費する。
 */
export const OP = {
  APPEND_CHILD: 0,
  INSERT_BEFORE: 1,
  REMOVE: 2,
  SET_ROOT: 3,
  SET_STYLE: 4,
  SET_TRANSFORM: 5,
  SET_SCROLL_OFFSET: 6,
  FOCUS: 7,
  BLUR: 8,
  CREATE: 9,
} as const;

/**
 * `ElementKind` → kind_code。Hayate の `element_kind_*()` が返す値と一致。
 * `OP_CREATE` の operand として ops ストリームに乗せる（Tsubame ADR-0005）。
 */
export const ELEMENT_KIND: Record<ElementKind, number> = {
  view: 0,
  text: 1,
  image: 2,
  button: 3,
  'text-input': 4,
  'scroll-view': 5,
};
