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
  out.push(tag, d.value, UNIT_CODE[d.unit]!);
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
        out.push(TAG.DISPLAY, DISPLAY_CODE[value as Display]!);
        break;
      case 'flexDirection':
        out.push(TAG.FLEX_DIRECTION, FLEX_DIRECTION_CODE[value as FlexDirection]!);
        break;
      case 'alignItems':
        out.push(TAG.ALIGN_ITEMS, ALIGN_ITEMS_CODE[value as AlignItems]!);
        break;
      case 'justifyContent':
        out.push(TAG.JUSTIFY_CONTENT, JUSTIFY_CONTENT_CODE[value as JustifyContent]!);
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
        throw new Error(`CanvasRenderer: unsupported style property "${String(k)}"`);
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
      throw new Error(`CanvasRenderer: cannot reset non-inheritable property "${String(k)}"`);
    }
    kinds.push(code);
  }
  return kinds;
}
