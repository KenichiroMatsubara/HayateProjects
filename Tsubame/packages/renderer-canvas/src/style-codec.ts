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
import { INHERITED_UNSET, STYLE_ENCODE_META } from '@tsubame/hayate-css-catalog';

export { TAG, UNSET_KIND };

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

type StyleEntryEncoder = (out: number[], value: NonNullable<StylePatch[keyof StylePatch]>) => void;

function buildEncoder(entry: (typeof STYLE_ENCODE_META)[number]): StyleEntryEncoder {
  const { tag, kind } = entry;
  switch (kind) {
    case 'color':
      return (out, v) => pushColor(out, tag, v as string);
    case 'dimension':
      return (out, v) => pushDimension(out, tag, v as HayateDimension);
    case 'display':
      return (out, v) => out.push(tag, DISPLAY_CODE[v as Display]!);
    case 'flexDirection':
      return (out, v) => out.push(tag, FLEX_DIRECTION_CODE[v as FlexDirection]!);
    case 'alignItems':
      return (out, v) => out.push(tag, ALIGN_ITEMS_CODE[v as AlignItems]!);
    case 'justifyContent':
      return (out, v) => out.push(tag, JUSTIFY_CONTENT_CODE[v as JustifyContent]!);
    case 'fontFamily':
      return (out, v) => pushFontFamily(out, String(v));
    case 'zIndex':
      return (out, v) => out.push(tag, finiteInteger('zIndex', v));
    case 'f32':
      return (out, v) => out.push(tag, finiteNumber(entry.key, v));
    default: {
      const _exhaustive: never = kind;
      throw new Error(`unsupported encode kind ${_exhaustive}`);
    }
  }
}

const STYLE_ENCODERS: Partial<Record<keyof StylePatch, StyleEntryEncoder>> = {};
for (const entry of STYLE_ENCODE_META) {
  const key = entry.key as keyof StylePatch;
  STYLE_ENCODERS[key] = buildEncoder(entry);
}

/**
 * `StylePatch` の SET 分を style-packet TAG 形式の f32 スロットとして
 * `out` に追記する（`null` の reset 分は {@link unsetKindsOf} が扱う）。
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
    const code = INHERITED_UNSET[k as string];
    if (code === undefined) {
      throw new Error(`CanvasRenderer: cannot reset non-inheritable property "${String(k)}"`);
    }
    kinds.push(code);
  }
  return kinds;
}
