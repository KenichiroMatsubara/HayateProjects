import type {
  AlignItems,
  Display,
  FlexDirection,
  HayateDimension,
  JustifyContent,
  StylePatch,
} from '@tsubame/renderer-protocol';
import {
  finiteInteger,
  finiteNumber,
  parseColor,
  parseDimension,
} from './hayate.js';
import {
  TAG,
  UNSET_KIND,
  UNIT_CODE,
  DISPLAY,
  FLEX_DIRECTION,
  ALIGN_ITEMS,
  JUSTIFY_CONTENT,
} from './protocol.js';

export { TAG, UNSET_KIND };

// ── Lookup tables ─────────────────────────────────────────────────────────────

const DISPLAY_CODE: Record<Display, number> = {
  flex: DISPLAY.flex,
  grid: DISPLAY.grid,
  block: DISPLAY.block,
  none: DISPLAY.none,
};

const FLEX_DIRECTION_CODE: Record<FlexDirection, number> = {
  row: FLEX_DIRECTION.row,
  column: FLEX_DIRECTION.column,
  'row-reverse': FLEX_DIRECTION.rowReverse,
  'column-reverse': FLEX_DIRECTION.columnReverse,
};

const ALIGN_ITEMS_CODE: Record<AlignItems, number> = {
  'flex-start': ALIGN_ITEMS.flexStart,
  'flex-end': ALIGN_ITEMS.flexEnd,
  center: ALIGN_ITEMS.center,
  stretch: ALIGN_ITEMS.stretch,
  baseline: ALIGN_ITEMS.baseline,
};

const JUSTIFY_CONTENT_CODE: Record<JustifyContent, number> = {
  'flex-start': JUSTIFY_CONTENT.flexStart,
  'flex-end': JUSTIFY_CONTENT.flexEnd,
  center: JUSTIFY_CONTENT.center,
  'space-between': JUSTIFY_CONTENT.spaceBetween,
  'space-around': JUSTIFY_CONTENT.spaceAround,
  'space-evenly': JUSTIFY_CONTENT.spaceEvenly,
};

/** Inheritable props (ADR-0047). A `null` value resets these to inherit. */
export const INHERITED_UNSET: Partial<Record<keyof StylePatch, number>> = {
  color: UNSET_KIND.color,
  fontSize: UNSET_KIND.fontSize,
  fontFamily: UNSET_KIND.fontFamily,
  fontWeight: UNSET_KIND.fontWeight,
};

// ── Primitive push helpers ────────────────────────────────────────────────────

function pushColor(out: number[], tag: number, css: string): void {
  const c = parseColor(css);
  out.push(tag, c.r, c.g, c.b, c.a);
}

function pushDimension(out: number[], tag: number, value: HayateDimension): void {
  const d = parseDimension(value);
  out.push(tag, d.value, UNIT_CODE[d.unit]!);
}

function pushFontFamily(out: number[], family: string): void {
  const bytes = new TextEncoder().encode(family);
  out.push(TAG.FONT_FAMILY, bytes.length);
  for (const byte of bytes) out.push(byte);
}

// ── CODEC table ───────────────────────────────────────────────────────────────

type StyleEntryEncoder = (out: number[], value: NonNullable<StylePatch[keyof StylePatch]>) => void;

const STYLE_ENCODERS: Partial<Record<keyof StylePatch, StyleEntryEncoder>> = {
  backgroundColor: (out, v) => pushColor(out, TAG.BACKGROUND_COLOR, v as string),
  borderColor:     (out, v) => pushColor(out, TAG.BORDER_COLOR, v as string),
  color:           (out, v) => pushColor(out, TAG.COLOR, v as string),
  opacity:         (out, v) => out.push(TAG.OPACITY, finiteNumber('opacity', v)),
  borderRadius:    (out, v) => out.push(TAG.BORDER_RADIUS, finiteNumber('borderRadius', v)),
  borderWidth:     (out, v) => out.push(TAG.BORDER_WIDTH, finiteNumber('borderWidth', v)),
  width:           (out, v) => pushDimension(out, TAG.WIDTH, v as HayateDimension),
  height:          (out, v) => pushDimension(out, TAG.HEIGHT, v as HayateDimension),
  minWidth:        (out, v) => pushDimension(out, TAG.MIN_WIDTH, v as HayateDimension),
  minHeight:       (out, v) => pushDimension(out, TAG.MIN_HEIGHT, v as HayateDimension),
  maxWidth:        (out, v) => pushDimension(out, TAG.MAX_WIDTH, v as HayateDimension),
  maxHeight:       (out, v) => pushDimension(out, TAG.MAX_HEIGHT, v as HayateDimension),
  display:         (out, v) => out.push(TAG.DISPLAY, DISPLAY_CODE[v as Display]!),
  flexDirection:   (out, v) => out.push(TAG.FLEX_DIRECTION, FLEX_DIRECTION_CODE[v as FlexDirection]!),
  alignItems:      (out, v) => out.push(TAG.ALIGN_ITEMS, ALIGN_ITEMS_CODE[v as AlignItems]!),
  justifyContent:  (out, v) => out.push(TAG.JUSTIFY_CONTENT, JUSTIFY_CONTENT_CODE[v as JustifyContent]!),
  gap:             (out, v) => pushDimension(out, TAG.GAP, v as HayateDimension),
  flexGrow:        (out, v) => out.push(TAG.FLEX_GROW, finiteNumber('flexGrow', v)),
  padding:         (out, v) => pushDimension(out, TAG.PADDING, v as HayateDimension),
  paddingTop:      (out, v) => pushDimension(out, TAG.PADDING_TOP, v as HayateDimension),
  paddingRight:    (out, v) => pushDimension(out, TAG.PADDING_RIGHT, v as HayateDimension),
  paddingBottom:   (out, v) => pushDimension(out, TAG.PADDING_BOTTOM, v as HayateDimension),
  paddingLeft:     (out, v) => pushDimension(out, TAG.PADDING_LEFT, v as HayateDimension),
  margin:          (out, v) => pushDimension(out, TAG.MARGIN, v as HayateDimension),
  marginTop:       (out, v) => pushDimension(out, TAG.MARGIN_TOP, v as HayateDimension),
  marginRight:     (out, v) => pushDimension(out, TAG.MARGIN_RIGHT, v as HayateDimension),
  marginBottom:    (out, v) => pushDimension(out, TAG.MARGIN_BOTTOM, v as HayateDimension),
  marginLeft:      (out, v) => pushDimension(out, TAG.MARGIN_LEFT, v as HayateDimension),
  fontSize:        (out, v) => out.push(TAG.FONT_SIZE, finiteNumber('fontSize', v)),
  fontFamily:      (out, v) => pushFontFamily(out, String(v)),
  fontWeight:      (out, v) => out.push(TAG.FONT_WEIGHT, finiteNumber('fontWeight', v)),
  zIndex:          (out, v) => out.push(TAG.Z_INDEX, finiteInteger('zIndex', v)),
};

// ── Public API ────────────────────────────────────────────────────────────────

/**
 * `StylePatch` の SET 分を style-packet TAG 形式の f32 スロットとして
 * `out` に追記する（`null` の reset 分は {@link unsetKindsOf} が扱う）。
 *
 * 共有バッファへ直接 append することで、`CanvasRenderer` がフレーム単位の
 * `styles` 配列に積み、`OP_SET_STYLE` の offset/len で参照できる（コピー回避）。
 */
export function encodeStylePatch(patch: StylePatch, out: number[]): void {
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined || value === null) continue;

    const encoder = STYLE_ENCODERS[k];
    if (encoder === undefined) {
      throw new Error(`CanvasRenderer: unsupported style property "${String(k)}"`);
    }
    encoder(out, value as NonNullable<StylePatch[keyof StylePatch]>);
  }
}

/**
 * `StylePatch` 内の `null` 値（リセット）を `OP_UNSET_STYLE` の kind コード列に変換する。
 * 継承プロパティ（ADR-0047）以外への `null` は throw。
 */
export function unsetKindsOf(patch: StylePatch): number[] {
  const kinds: number[] = [];
  for (const key in patch) {
    const k = key as keyof StylePatch;
    if (patch[k] !== null) continue;
    const code = INHERITED_UNSET[k];
    if (code === undefined) {
      throw new Error(`CanvasRenderer: cannot reset non-inheritable property "${String(k)}"`);
    }
    kinds.push(code);
  }
  return kinds;
}
