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
  type HayateDimensionUnit,
} from './hayate.js';

/**
 * style-packet の TAG 定数。
 *
 * Hayate `crates/adapters/web/src/style_packet.rs` の `TAG_*` と 1:1 で
 * 一致させること。実 WASM の `element_set_style` / `apply_mutations` は
 * この flat f32 エンコーディングを `decode()` でデコードする（ADR-0039）。
 */
export const TAG = {
  BACKGROUND_COLOR: 0,
  OPACITY: 1,
  BORDER_RADIUS: 2,
  BORDER_WIDTH: 3,
  BORDER_COLOR: 4,
  WIDTH: 5,
  HEIGHT: 6,
  MIN_WIDTH: 7,
  MIN_HEIGHT: 8,
  MAX_WIDTH: 9,
  MAX_HEIGHT: 10,
  DISPLAY: 11,
  FLEX_DIRECTION: 12,
  ALIGN_ITEMS: 13,
  JUSTIFY_CONTENT: 14,
  GAP: 15,
  PADDING: 16,
  PADDING_TOP: 17,
  PADDING_RIGHT: 18,
  PADDING_BOTTOM: 19,
  PADDING_LEFT: 20,
  MARGIN: 21,
  MARGIN_TOP: 22,
  MARGIN_RIGHT: 23,
  MARGIN_BOTTOM: 24,
  MARGIN_LEFT: 25,
  FONT_SIZE: 26,
  COLOR: 27,
  Z_INDEX: 28,
  FONT_FAMILY: 29,
  FLEX_GROW: 30,
  FONT_WEIGHT: 31,
} as const;

/**
 * `element_unset_style` の kind コード（Hayate `element_renderer.rs` と一致）。
 * 継承プロパティのみリセット可能（ADR-0047）。
 */
export const UNSET_KIND = {
  color: 0,
  fontSize: 1,
  fontFamily: 2,
  fontWeight: 3,
} as const;

const UNIT_CODE: Record<HayateDimensionUnit, number> = {
  px: 0,
  percent: 1,
  auto: 2,
  fr: 3,
};

const DISPLAY_CODE: Record<Display, number> = {
  flex: 0,
  grid: 1,
  block: 2,
  none: 3,
};

const FLEX_DIRECTION_CODE: Record<FlexDirection, number> = {
  row: 0,
  column: 1,
  'row-reverse': 2,
  'column-reverse': 3,
};

const ALIGN_ITEMS_CODE: Record<AlignItems, number> = {
  'flex-start': 0,
  'flex-end': 1,
  center: 2,
  stretch: 3,
  baseline: 4,
};

const JUSTIFY_CONTENT_CODE: Record<JustifyContent, number> = {
  'flex-start': 0,
  'flex-end': 1,
  center: 2,
  'space-between': 3,
  'space-around': 4,
  'space-evenly': 5,
};

/** Inheritable props (ADR-0047). A `null` value resets these to inherit. */
const INHERITED_UNSET: Partial<Record<keyof StylePatch, number>> = {
  color: UNSET_KIND.color,
  fontSize: UNSET_KIND.fontSize,
  fontFamily: UNSET_KIND.fontFamily,
  fontWeight: UNSET_KIND.fontWeight,
};

function pushColor(out: number[], tag: number, css: string): void {
  const c = parseColor(css);
  out.push(tag, c.r, c.g, c.b, c.a);
}

function pushDimension(out: number[], tag: number, value: HayateDimension): void {
  const d = parseDimension(value);
  out.push(tag, d.value, UNIT_CODE[d.unit]);
}

function pushFontFamily(out: number[], family: string): void {
  const bytes = new TextEncoder().encode(family);
  out.push(TAG.FONT_FAMILY, bytes.length);
  for (const byte of bytes) out.push(byte);
}

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

    switch (k) {
      case 'backgroundColor':
        pushColor(out, TAG.BACKGROUND_COLOR, value as string);
        break;
      case 'borderColor':
        pushColor(out, TAG.BORDER_COLOR, value as string);
        break;
      case 'color':
        pushColor(out, TAG.COLOR, value as string);
        break;
      case 'opacity':
        out.push(TAG.OPACITY, finiteNumber(k, value));
        break;
      case 'borderRadius':
        out.push(TAG.BORDER_RADIUS, finiteNumber(k, value));
        break;
      case 'borderWidth':
        out.push(TAG.BORDER_WIDTH, finiteNumber(k, value));
        break;
      case 'width':
        pushDimension(out, TAG.WIDTH, value as HayateDimension);
        break;
      case 'height':
        pushDimension(out, TAG.HEIGHT, value as HayateDimension);
        break;
      case 'minWidth':
        pushDimension(out, TAG.MIN_WIDTH, value as HayateDimension);
        break;
      case 'minHeight':
        pushDimension(out, TAG.MIN_HEIGHT, value as HayateDimension);
        break;
      case 'maxWidth':
        pushDimension(out, TAG.MAX_WIDTH, value as HayateDimension);
        break;
      case 'maxHeight':
        pushDimension(out, TAG.MAX_HEIGHT, value as HayateDimension);
        break;
      case 'display':
        out.push(TAG.DISPLAY, DISPLAY_CODE[value as Display]);
        break;
      case 'flexDirection':
        out.push(TAG.FLEX_DIRECTION, FLEX_DIRECTION_CODE[value as FlexDirection]);
        break;
      case 'alignItems':
        out.push(TAG.ALIGN_ITEMS, ALIGN_ITEMS_CODE[value as AlignItems]);
        break;
      case 'justifyContent':
        out.push(TAG.JUSTIFY_CONTENT, JUSTIFY_CONTENT_CODE[value as JustifyContent]);
        break;
      case 'gap':
        pushDimension(out, TAG.GAP, value as HayateDimension);
        break;
      case 'flexGrow':
        out.push(TAG.FLEX_GROW, finiteNumber(k, value));
        break;
      case 'padding':
        pushDimension(out, TAG.PADDING, value as HayateDimension);
        break;
      case 'paddingTop':
        pushDimension(out, TAG.PADDING_TOP, value as HayateDimension);
        break;
      case 'paddingRight':
        pushDimension(out, TAG.PADDING_RIGHT, value as HayateDimension);
        break;
      case 'paddingBottom':
        pushDimension(out, TAG.PADDING_BOTTOM, value as HayateDimension);
        break;
      case 'paddingLeft':
        pushDimension(out, TAG.PADDING_LEFT, value as HayateDimension);
        break;
      case 'margin':
        pushDimension(out, TAG.MARGIN, value as HayateDimension);
        break;
      case 'marginTop':
        pushDimension(out, TAG.MARGIN_TOP, value as HayateDimension);
        break;
      case 'marginRight':
        pushDimension(out, TAG.MARGIN_RIGHT, value as HayateDimension);
        break;
      case 'marginBottom':
        pushDimension(out, TAG.MARGIN_BOTTOM, value as HayateDimension);
        break;
      case 'marginLeft':
        pushDimension(out, TAG.MARGIN_LEFT, value as HayateDimension);
        break;
      case 'fontSize':
        out.push(TAG.FONT_SIZE, finiteNumber(k, value));
        break;
      case 'fontFamily':
        pushFontFamily(out, String(value));
        break;
      case 'fontWeight':
        out.push(TAG.FONT_WEIGHT, finiteNumber(k, value));
        break;
      case 'zIndex':
        out.push(TAG.Z_INDEX, finiteInteger(k, value));
        break;
      default:
        throw new Error(`CanvasRenderer: unsupported style property "${k}"`);
    }
  }
}

/**
 * `StylePatch` 内の `null` 値（リセット）を `element_unset_style` の
 * kind コード列に変換する。継承プロパティ（ADR-0047）以外への `null` は throw。
 */
export function unsetKindsOf(patch: StylePatch): number[] {
  const kinds: number[] = [];
  for (const key in patch) {
    const k = key as keyof StylePatch;
    if (patch[k] !== null) continue;
    const code = INHERITED_UNSET[k];
    if (code === undefined) {
      throw new Error(`CanvasRenderer: cannot reset non-inheritable property "${k}"`);
    }
    kinds.push(code);
  }
  return kinds;
}
