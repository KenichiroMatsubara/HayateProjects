// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @torimi/hayate-protocol-spec

import type { StylePatch } from '@torimi/tsubame-renderer-protocol';
import { OP, DRAW_OP, DRAW_PAINT_FIELD, TAG, UNSET_KIND, UNIT_CODE, DISPLAY, FLEX_DIRECTION, FLEX_WRAP, ALIGN_ITEMS, ALIGN_SELF, ALIGN_CONTENT, JUSTIFY_CONTENT, FONT_STYLE, TEXT_DECORATION, BORDER_STYLE, CURSOR, OVERFLOW, TEXT_OVERFLOW, POSITION, TRANSITION_TIMING, BOX_SIZING, GRID_AUTO_FLOW, JUSTIFY_ITEMS, JUSTIFY_SELF } from './protocol.js';

export { TAG, UNSET_KIND } from './protocol.js';

export type HayateDimensionUnit = 'px' | 'percent' | 'auto' | 'fr';

export interface HayateDimensionRecord {
  value: number;
  unit: HayateDimensionUnit;
}

export interface HayateColorRecord {
  r: number;
  g: number;
  b: number;
  a: number;
}

export function finiteNumber(key: string, value: unknown): number {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    throw new Error(`HayateRenderer: invalid numeric value for "${key}"`);
  }
  return numeric;
}

export function finiteInteger(key: string, value: unknown): number {
  const numeric = finiteNumber(key, value);
  if (!Number.isInteger(numeric)) {
    throw new Error(`HayateRenderer: "${key}" must be an integer`);
  }
  return numeric;
}

export function parseDimension(value: import('@torimi/tsubame-renderer-protocol').HayateDimension): HayateDimensionRecord {
  if (typeof value === 'number') {
    return { value, unit: 'px' };
  }

  const trimmed = value.trim().toLowerCase();
  if (trimmed === 'auto') {
    return { value: 0, unit: 'auto' };
  }

  const match = trimmed.match(/^(-?(?:\d+|\d*\.\d+))(px|%|fr)?$/);
  if (match === null) {
    throw new Error(`HayateRenderer: unsupported dimension "${value}"`);
  }

  const numeric = Number(match[1]);
  if (!Number.isFinite(numeric)) {
    throw new Error(`HayateRenderer: invalid dimension "${value}"`);
  }

  const unit = match[2] ?? 'px';
  if (unit === '%') return { value: numeric, unit: 'percent' };
  if (unit === 'fr') return { value: numeric, unit: 'fr' };
  return { value: numeric, unit: 'px' };
}

export function parseColor(input: string): HayateColorRecord {
  const s = input.trim().toLowerCase();
  if (s.startsWith('#')) {
    const hex = s.slice(1);
    const read1 = (i: number): number => parseInt(hex[i]! + hex[i]!, 16) / 255;
    const read2 = (i: number): number => parseInt(hex.slice(i, i + 2), 16) / 255;
    if (hex.length === 3) return { r: read1(0), g: read1(1), b: read1(2), a: 1 };
    if (hex.length === 4) return { r: read1(0), g: read1(1), b: read1(2), a: read1(3) };
    if (hex.length === 6) return { r: read2(0), g: read2(2), b: read2(4), a: 1 };
    if (hex.length === 8) return { r: read2(0), g: read2(2), b: read2(4), a: read2(6) };
  }

  const rgb = s.match(/^rgba?\((.*)\)$/);
  if (rgb !== null) {
    const normalized = rgb[1]!.replace(/\s*\/\s*/, ',').replace(/\s+/g, ',');
    const parts = normalized.split(',').filter(Boolean);
    if (parts.length >= 3) {
      return {
        r: parseColorChannel(parts[0]!),
        g: parseColorChannel(parts[1]!),
        b: parseColorChannel(parts[2]!),
        a: parts[3] === undefined ? 1 : parseAlpha(parts[3]),
      };
    }
  }

  if (s === 'transparent') {
    return { r: 0, g: 0, b: 0, a: 0 };
  }

  throw new Error(`HayateRenderer: unsupported color "${input}"`);
}

function parseColorChannel(raw: string): number {
  const value = raw.trim();
  if (value.endsWith('%')) return clamp01(parseFloat(value) / 100);
  return clamp01(parseFloat(value) / 255);
}

function parseAlpha(raw: string): number {
  const value = raw.trim();
  if (value.endsWith('%')) return clamp01(parseFloat(value) / 100);
  return clamp01(parseFloat(value));
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}

/**
 * grid-placement の1スロット（start または end）を [種別タグ, 整数] の2 wire
 * スロットへ符号化する。`auto`/undefined は `[0, 0]`、line(整数) は `[1, n]`、
 * span は `{ span: n }` → `[2, n]`。
 */
export function encodeGridLine(out: number[], key: string, line: unknown): void {
  if (line === undefined || line === null || line === 'auto') {
    out.push(0, 0);
    return;
  }
  if (typeof line === 'number') {
    out.push(1, finiteInteger(key, line));
    return;
  }
  if (typeof line === 'object' && 'span' in (line as Record<string, unknown>)) {
    out.push(2, finiteInteger(`${key}.span`, (line as { span: unknown }).span));
    return;
  }
  throw new Error(`HayateRenderer: unsupported grid placement for "${key}"`);
}

const DISPLAY_CODE: Record<string, number> = {
  'flex': DISPLAY.flex,
  'grid': DISPLAY.grid,
  'block': DISPLAY.block,
  'none': DISPLAY.none,
};

const FLEX_DIRECTION_CODE: Record<string, number> = {
  'row': FLEX_DIRECTION.row,
  'column': FLEX_DIRECTION.column,
  'row-reverse': FLEX_DIRECTION.rowReverse,
  'column-reverse': FLEX_DIRECTION.columnReverse,
};

const FLEX_WRAP_CODE: Record<string, number> = {
  'nowrap': FLEX_WRAP.nowrap,
  'wrap': FLEX_WRAP.wrap,
  'wrap-reverse': FLEX_WRAP.wrapReverse,
};

const ALIGN_ITEMS_CODE: Record<string, number> = {
  'flex-start': ALIGN_ITEMS.flexStart,
  'flex-end': ALIGN_ITEMS.flexEnd,
  'center': ALIGN_ITEMS.center,
  'stretch': ALIGN_ITEMS.stretch,
  'baseline': ALIGN_ITEMS.baseline,
};

const ALIGN_SELF_CODE: Record<string, number> = {
  'auto': ALIGN_SELF.auto,
  'flex-start': ALIGN_SELF.flexStart,
  'flex-end': ALIGN_SELF.flexEnd,
  'center': ALIGN_SELF.center,
  'stretch': ALIGN_SELF.stretch,
  'baseline': ALIGN_SELF.baseline,
};

const ALIGN_CONTENT_CODE: Record<string, number> = {
  'flex-start': ALIGN_CONTENT.flexStart,
  'flex-end': ALIGN_CONTENT.flexEnd,
  'center': ALIGN_CONTENT.center,
  'stretch': ALIGN_CONTENT.stretch,
  'space-between': ALIGN_CONTENT.spaceBetween,
  'space-around': ALIGN_CONTENT.spaceAround,
  'space-evenly': ALIGN_CONTENT.spaceEvenly,
};

const JUSTIFY_CONTENT_CODE: Record<string, number> = {
  'flex-start': JUSTIFY_CONTENT.flexStart,
  'flex-end': JUSTIFY_CONTENT.flexEnd,
  'center': JUSTIFY_CONTENT.center,
  'space-between': JUSTIFY_CONTENT.spaceBetween,
  'space-around': JUSTIFY_CONTENT.spaceAround,
  'space-evenly': JUSTIFY_CONTENT.spaceEvenly,
};

const FONT_STYLE_CODE: Record<string, number> = {
  'normal': FONT_STYLE.normal,
  'italic': FONT_STYLE.italic,
  'oblique': FONT_STYLE.oblique,
};

const TEXT_DECORATION_CODE: Record<string, number> = {
  'none': TEXT_DECORATION.none,
  'underline': TEXT_DECORATION.underline,
  'line-through': TEXT_DECORATION.lineThrough,
};

const BORDER_STYLE_CODE: Record<string, number> = {
  'none': BORDER_STYLE.none,
  'solid': BORDER_STYLE.solid,
  'dashed': BORDER_STYLE.dashed,
};

const CURSOR_CODE: Record<string, number> = {
  'default': CURSOR.default,
  'pointer': CURSOR.pointer,
  'text': CURSOR.text,
  'crosshair': CURSOR.crosshair,
  'not-allowed': CURSOR.notAllowed,
  'grab': CURSOR.grab,
  'grabbing': CURSOR.grabbing,
};

const OVERFLOW_CODE: Record<string, number> = {
  'visible': OVERFLOW.visible,
  'hidden': OVERFLOW.hidden,
};

const TEXT_OVERFLOW_CODE: Record<string, number> = {
  'clip': TEXT_OVERFLOW.clip,
  'ellipsis': TEXT_OVERFLOW.ellipsis,
};

const POSITION_CODE: Record<string, number> = {
  'relative': POSITION.relative,
  'absolute': POSITION.absolute,
};

const TRANSITION_TIMING_CODE: Record<string, number> = {
  'ease': TRANSITION_TIMING.ease,
  'linear': TRANSITION_TIMING.linear,
  'ease-in': TRANSITION_TIMING.easeIn,
  'ease-out': TRANSITION_TIMING.easeOut,
  'ease-in-out': TRANSITION_TIMING.easeInOut,
};

const BOX_SIZING_CODE: Record<string, number> = {
  'border-box': BOX_SIZING.borderBox,
  'content-box': BOX_SIZING.contentBox,
};

const GRID_AUTO_FLOW_CODE: Record<string, number> = {
  'row': GRID_AUTO_FLOW.row,
  'column': GRID_AUTO_FLOW.column,
  'row-dense': GRID_AUTO_FLOW.rowDense,
  'column-dense': GRID_AUTO_FLOW.columnDense,
};

const JUSTIFY_ITEMS_CODE: Record<string, number> = {
  'start': JUSTIFY_ITEMS.start,
  'end': JUSTIFY_ITEMS.end,
  'center': JUSTIFY_ITEMS.center,
  'stretch': JUSTIFY_ITEMS.stretch,
};

const JUSTIFY_SELF_CODE: Record<string, number> = {
  'auto': JUSTIFY_SELF.auto,
  'start': JUSTIFY_SELF.start,
  'end': JUSTIFY_SELF.end,
  'center': JUSTIFY_SELF.center,
  'stretch': JUSTIFY_SELF.stretch,
};

function encode_backgroundColor(out: number[], value: string): void {
  const c = parseColor(value);
  out.push(TAG.BACKGROUND_COLOR, c.r, c.g, c.b, c.a);
}

function encode_opacity(out: number[], value: unknown): void {
  out.push(TAG.OPACITY, finiteNumber('opacity', value));
}

function encode_borderRadius(out: number[], value: unknown): void {
  out.push(TAG.BORDER_RADIUS, finiteNumber('borderRadius', value));
}

function encode_borderWidth(out: number[], value: unknown): void {
  out.push(TAG.BORDER_WIDTH, finiteNumber('borderWidth', value));
}

function encode_borderColor(out: number[], value: string): void {
  const c = parseColor(value);
  out.push(TAG.BORDER_COLOR, c.r, c.g, c.b, c.a);
}

function encode_width(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.WIDTH, d.value, UNIT_CODE[d.unit]!);
}

function encode_height(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.HEIGHT, d.value, UNIT_CODE[d.unit]!);
}

function encode_minWidth(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MIN_WIDTH, d.value, UNIT_CODE[d.unit]!);
}

function encode_minHeight(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MIN_HEIGHT, d.value, UNIT_CODE[d.unit]!);
}

function encode_maxWidth(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MAX_WIDTH, d.value, UNIT_CODE[d.unit]!);
}

function encode_maxHeight(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MAX_HEIGHT, d.value, UNIT_CODE[d.unit]!);
}

function encode_display(out: number[], value: string): void {
  const code = DISPLAY_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported display "${value}"`);
  out.push(TAG.DISPLAY, code);
}

function encode_flexDirection(out: number[], value: string): void {
  const code = FLEX_DIRECTION_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported flexDirection "${value}"`);
  out.push(TAG.FLEX_DIRECTION, code);
}

function encode_alignItems(out: number[], value: string): void {
  const code = ALIGN_ITEMS_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported alignItems "${value}"`);
  out.push(TAG.ALIGN_ITEMS, code);
}

function encode_justifyContent(out: number[], value: string): void {
  const code = JUSTIFY_CONTENT_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported justifyContent "${value}"`);
  out.push(TAG.JUSTIFY_CONTENT, code);
}

function encode_gap(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.GAP, d.value, UNIT_CODE[d.unit]!);
}

function encode_padding(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.PADDING, d.value, UNIT_CODE[d.unit]!);
}

function encode_paddingTop(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.PADDING_TOP, d.value, UNIT_CODE[d.unit]!);
}

function encode_paddingRight(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.PADDING_RIGHT, d.value, UNIT_CODE[d.unit]!);
}

function encode_paddingBottom(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.PADDING_BOTTOM, d.value, UNIT_CODE[d.unit]!);
}

function encode_paddingLeft(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.PADDING_LEFT, d.value, UNIT_CODE[d.unit]!);
}

function encode_margin(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MARGIN, d.value, UNIT_CODE[d.unit]!);
}

function encode_marginTop(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MARGIN_TOP, d.value, UNIT_CODE[d.unit]!);
}

function encode_marginRight(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MARGIN_RIGHT, d.value, UNIT_CODE[d.unit]!);
}

function encode_marginBottom(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MARGIN_BOTTOM, d.value, UNIT_CODE[d.unit]!);
}

function encode_marginLeft(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.MARGIN_LEFT, d.value, UNIT_CODE[d.unit]!);
}

function encode_fontSize(out: number[], value: unknown): void {
  out.push(TAG.FONT_SIZE, finiteNumber('fontSize', value));
}

function encode_color(out: number[], value: string): void {
  const c = parseColor(value);
  out.push(TAG.COLOR, c.r, c.g, c.b, c.a);
}

function encode_zIndex(out: number[], value: unknown): void {
  out.push(TAG.Z_INDEX, finiteInteger('zIndex', value));
}

function encode_fontFamily(out: number[], value: string): void {
  const bytes = new TextEncoder().encode(value);
  out.push(TAG.FONT_FAMILY, bytes.length);
  for (const byte of bytes) out.push(byte);
}

function encode_flexGrow(out: number[], value: unknown): void {
  out.push(TAG.FLEX_GROW, finiteNumber('flexGrow', value));
}

function encode_fontWeight(out: number[], value: unknown): void {
  out.push(TAG.FONT_WEIGHT, finiteNumber('fontWeight', value));
}

function encode_fontStyle(out: number[], value: string): void {
  const code = FONT_STYLE_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported fontStyle "${value}"`);
  out.push(TAG.FONT_STYLE, code);
}

function encode_textDecoration(out: number[], value: string): void {
  const code = TEXT_DECORATION_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported textDecoration "${value}"`);
  out.push(TAG.TEXT_DECORATION, code);
}

function encode_defaultColor(out: number[], value: string): void {
  const c = parseColor(value);
  out.push(TAG.DEFAULT_COLOR, c.r, c.g, c.b, c.a);
}

function encode_defaultFontFamily(out: number[], value: string): void {
  const bytes = new TextEncoder().encode(value);
  out.push(TAG.DEFAULT_FONT_FAMILY, bytes.length);
  for (const byte of bytes) out.push(byte);
}

function encode_defaultFontSize(out: number[], value: unknown): void {
  out.push(TAG.DEFAULT_FONT_SIZE, finiteNumber('defaultFontSize', value));
}

function encode_defaultFontWeight(out: number[], value: unknown): void {
  out.push(TAG.DEFAULT_FONT_WEIGHT, finiteNumber('defaultFontWeight', value));
}

function encode_gridTemplateColumns(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension[]): void {
  if (!Array.isArray(value)) {
    throw new Error(`HayateRenderer: "gridTemplateColumns" must be an array of dimensions`);
  }
  out.push(TAG.GRID_TEMPLATE_COLUMNS, value.length);
  for (const item of value) {
    const d = parseDimension(item);
    out.push(d.value, UNIT_CODE[d.unit]!);
  }
}

function encode_gridTemplateRows(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension[]): void {
  if (!Array.isArray(value)) {
    throw new Error(`HayateRenderer: "gridTemplateRows" must be an array of dimensions`);
  }
  out.push(TAG.GRID_TEMPLATE_ROWS, value.length);
  for (const item of value) {
    const d = parseDimension(item);
    out.push(d.value, UNIT_CODE[d.unit]!);
  }
}

function encode_flexShrink(out: number[], value: unknown): void {
  out.push(TAG.FLEX_SHRINK, finiteNumber('flexShrink', value));
}

function encode_flexBasis(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.FLEX_BASIS, d.value, UNIT_CODE[d.unit]!);
}

function encode_alignSelf(out: number[], value: string): void {
  const code = ALIGN_SELF_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported alignSelf "${value}"`);
  out.push(TAG.ALIGN_SELF, code);
}

function encode_alignContent(out: number[], value: string): void {
  const code = ALIGN_CONTENT_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported alignContent "${value}"`);
  out.push(TAG.ALIGN_CONTENT, code);
}

function encode_flexWrap(out: number[], value: string): void {
  const code = FLEX_WRAP_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported flexWrap "${value}"`);
  out.push(TAG.FLEX_WRAP, code);
}

function encode_borderStyle(out: number[], value: string): void {
  const code = BORDER_STYLE_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported borderStyle "${value}"`);
  out.push(TAG.BORDER_STYLE, code);
}

function encode_cursor(out: number[], value: string): void {
  const code = CURSOR_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported cursor "${value}"`);
  out.push(TAG.CURSOR, code);
}

function encode_position(out: number[], value: string): void {
  const code = POSITION_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported position "${value}"`);
  out.push(TAG.POSITION, code);
}

function encode_top(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.TOP, d.value, UNIT_CODE[d.unit]!);
}

function encode_left(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.LEFT, d.value, UNIT_CODE[d.unit]!);
}

function encode_right(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.RIGHT, d.value, UNIT_CODE[d.unit]!);
}

function encode_bottom(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension): void {
  const d = parseDimension(value);
  out.push(TAG.BOTTOM, d.value, UNIT_CODE[d.unit]!);
}

function encode_overflow(out: number[], value: string): void {
  const code = OVERFLOW_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported overflow "${value}"`);
  out.push(TAG.OVERFLOW, code);
}

function encode_maxLines(out: number[], value: unknown): void {
  out.push(TAG.MAX_LINES, finiteInteger('maxLines', value));
}

function encode_textOverflow(out: number[], value: string): void {
  const code = TEXT_OVERFLOW_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported textOverflow "${value}"`);
  out.push(TAG.TEXT_OVERFLOW, code);
}

function encode_transitionDuration(out: number[], value: unknown): void {
  out.push(TAG.TRANSITION_DURATION, finiteNumber('transitionDuration', value));
}

function encode_transitionTiming(out: number[], value: string): void {
  const code = TRANSITION_TIMING_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported transitionTiming "${value}"`);
  out.push(TAG.TRANSITION_TIMING, code);
}

function encode_boxShadow(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateShadow[]): void {
  if (!Array.isArray(value)) {
    throw new Error(`HayateRenderer: "boxShadow" must be an array of shadows`);
  }
  out.push(TAG.BOX_SHADOW, value.length);
  for (const item of value) {
    const c = parseColor(item.color);
    out.push(
      finiteNumber('boxShadow.offsetX', item.offsetX),
      finiteNumber('boxShadow.offsetY', item.offsetY),
      finiteNumber('boxShadow.blur', item.blur),
      finiteNumber('boxShadow.spread', item.spread),
      c.r, c.g, c.b, c.a,
      item.inset ? 1 : 0,
    );
  }
}

function encode_aspectRatio(out: number[], value: unknown): void {
  out.push(TAG.ASPECT_RATIO, finiteNumber('aspectRatio', value));
}

function encode_boxSizing(out: number[], value: string): void {
  const code = BOX_SIZING_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported boxSizing "${value}"`);
  out.push(TAG.BOX_SIZING, code);
}

function encode_gridAutoRows(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension[]): void {
  if (!Array.isArray(value)) {
    throw new Error(`HayateRenderer: "gridAutoRows" must be an array of dimensions`);
  }
  out.push(TAG.GRID_AUTO_ROWS, value.length);
  for (const item of value) {
    const d = parseDimension(item);
    out.push(d.value, UNIT_CODE[d.unit]!);
  }
}

function encode_gridAutoColumns(out: number[], value: import('@torimi/tsubame-renderer-protocol').HayateDimension[]): void {
  if (!Array.isArray(value)) {
    throw new Error(`HayateRenderer: "gridAutoColumns" must be an array of dimensions`);
  }
  out.push(TAG.GRID_AUTO_COLUMNS, value.length);
  for (const item of value) {
    const d = parseDimension(item);
    out.push(d.value, UNIT_CODE[d.unit]!);
  }
}

function encode_gridAutoFlow(out: number[], value: string): void {
  const code = GRID_AUTO_FLOW_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported gridAutoFlow "${value}"`);
  out.push(TAG.GRID_AUTO_FLOW, code);
}

function encode_gridColumn(out: number[], value: unknown): void {
  const placement = (value ?? {}) as { start?: unknown; end?: unknown };
  out.push(TAG.GRID_COLUMN);
  encodeGridLine(out, 'gridColumn', placement.start);
  encodeGridLine(out, 'gridColumn', placement.end);
}

function encode_justifyItems(out: number[], value: string): void {
  const code = JUSTIFY_ITEMS_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported justifyItems "${value}"`);
  out.push(TAG.JUSTIFY_ITEMS, code);
}

function encode_justifySelf(out: number[], value: string): void {
  const code = JUSTIFY_SELF_CODE[value];
  if (code === undefined) throw new Error(`HayateRenderer: unsupported justifySelf "${value}"`);
  out.push(TAG.JUSTIFY_SELF, code);
}

function encode_gridRow(out: number[], value: unknown): void {
  const placement = (value ?? {}) as { start?: unknown; end?: unknown };
  out.push(TAG.GRID_ROW);
  encodeGridLine(out, 'gridRow', placement.start);
  encodeGridLine(out, 'gridRow', placement.end);
}

const STYLE_ENCODERS = {
  backgroundColor: encode_backgroundColor,
  opacity: encode_opacity,
  borderRadius: encode_borderRadius,
  borderWidth: encode_borderWidth,
  borderColor: encode_borderColor,
  width: encode_width,
  height: encode_height,
  minWidth: encode_minWidth,
  minHeight: encode_minHeight,
  maxWidth: encode_maxWidth,
  maxHeight: encode_maxHeight,
  display: encode_display,
  flexDirection: encode_flexDirection,
  alignItems: encode_alignItems,
  justifyContent: encode_justifyContent,
  gap: encode_gap,
  padding: encode_padding,
  paddingTop: encode_paddingTop,
  paddingRight: encode_paddingRight,
  paddingBottom: encode_paddingBottom,
  paddingLeft: encode_paddingLeft,
  margin: encode_margin,
  marginTop: encode_marginTop,
  marginRight: encode_marginRight,
  marginBottom: encode_marginBottom,
  marginLeft: encode_marginLeft,
  fontSize: encode_fontSize,
  color: encode_color,
  zIndex: encode_zIndex,
  fontFamily: encode_fontFamily,
  flexGrow: encode_flexGrow,
  fontWeight: encode_fontWeight,
  fontStyle: encode_fontStyle,
  textDecoration: encode_textDecoration,
  defaultColor: encode_defaultColor,
  defaultFontFamily: encode_defaultFontFamily,
  defaultFontSize: encode_defaultFontSize,
  defaultFontWeight: encode_defaultFontWeight,
  gridTemplateColumns: encode_gridTemplateColumns,
  gridTemplateRows: encode_gridTemplateRows,
  flexShrink: encode_flexShrink,
  flexBasis: encode_flexBasis,
  alignSelf: encode_alignSelf,
  alignContent: encode_alignContent,
  flexWrap: encode_flexWrap,
  borderStyle: encode_borderStyle,
  cursor: encode_cursor,
  position: encode_position,
  top: encode_top,
  left: encode_left,
  right: encode_right,
  bottom: encode_bottom,
  overflow: encode_overflow,
  maxLines: encode_maxLines,
  textOverflow: encode_textOverflow,
  transitionDuration: encode_transitionDuration,
  transitionTiming: encode_transitionTiming,
  boxShadow: encode_boxShadow,
  aspectRatio: encode_aspectRatio,
  boxSizing: encode_boxSizing,
  gridAutoRows: encode_gridAutoRows,
  gridAutoColumns: encode_gridAutoColumns,
  gridAutoFlow: encode_gridAutoFlow,
  gridColumn: encode_gridColumn,
  justifyItems: encode_justifyItems,
  justifySelf: encode_justifySelf,
  gridRow: encode_gridRow,
} as Partial<Record<keyof StylePatch, (out: number[], value: unknown) => void>>;

const INHERITED_UNSET: Partial<Record<string, number>> = {
  color: UNSET_KIND.color,
  fontSize: UNSET_KIND.fontSize,
  fontFamily: UNSET_KIND.fontFamily,
  fontWeight: UNSET_KIND.fontWeight,
};

/** StylePatch の SET 部分を style-packet の TAG ワイヤースロットへエンコードする。 */
export function encodeStylePatch(patch: StylePatch, out: number[]): void {
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined || value === null) continue;
    const encoder = STYLE_ENCODERS[k];
    if (encoder === undefined) {
      throw new Error(`HayateRenderer: unsupported style property "${String(k)}"`);
    }
    encoder(out, value);
  }
}

/** StylePatch 内の継承プロパティの null リセットを OP_UNSET_STYLE の種別コードへ対応付ける。 */
export function unsetKindsOf(patch: StylePatch): number[] {
  const kinds: number[] = [];
  for (const key in patch) {
    const k = key as keyof StylePatch;
    if (patch[k] !== null) continue;
    const code = INHERITED_UNSET[k as string];
    if (code === undefined) {
      throw new Error(`HayateRenderer: cannot reset non-inheritable property "${String(k)}"`);
    }
    kinds.push(code);
  }
  return kinds;
}

export function appendChild(buf: number[], parentId: number, childId: number): void {
  buf.push(OP.APPEND_CHILD);
  buf.push(parentId);
  buf.push(childId);
}

export function insertBefore(buf: number[], parentId: number, childId: number, beforeId: number): void {
  buf.push(OP.INSERT_BEFORE);
  buf.push(parentId);
  buf.push(childId);
  buf.push(beforeId);
}

export function appendRemove(buf: number[], id: number): void {
  buf.push(OP.REMOVE);
  buf.push(id);
}

export function appendSetRoot(buf: number[], id: number): void {
  buf.push(OP.SET_ROOT);
  buf.push(id);
}

export function appendSetStyle(buf: number[], id: number, styleOffset: number, styleLen: number): void {
  buf.push(OP.SET_STYLE);
  buf.push(id);
  buf.push(styleOffset);
  buf.push(styleLen);
}

export function appendSetTransform(buf: number[], id: number, hasMatrix: number, matrix: number[]): void {
  buf.push(OP.SET_TRANSFORM);
  buf.push(id);
  buf.push(hasMatrix);
  for (const slot of matrix) buf.push(slot);
}

export function appendSetScrollOffset(buf: number[], id: number, x: number, y: number): void {
  buf.push(OP.SET_SCROLL_OFFSET);
  buf.push(id);
  buf.push(x);
  buf.push(y);
}

export function appendFocus(buf: number[], id: number): void {
  buf.push(OP.FOCUS);
  buf.push(id);
}

export function appendBlur(buf: number[], id: number): void {
  buf.push(OP.BLUR);
  buf.push(id);
}

export function appendCreate(buf: number[], id: number, kind: number): void {
  buf.push(OP.CREATE);
  buf.push(id);
  buf.push(kind);
}

export function appendSetText(buf: number[], id: number, textIndex: number): void {
  buf.push(OP.SET_TEXT);
  buf.push(id);
  buf.push(textIndex);
}

export function appendUnsetStyle(buf: number[], id: number, kind: number): void {
  buf.push(OP.UNSET_STYLE);
  buf.push(id);
  buf.push(kind);
}

export function appendSetTextContent(buf: number[], id: number, textIndex: number): void {
  buf.push(OP.SET_TEXT_CONTENT);
  buf.push(id);
  buf.push(textIndex);
}

export function appendSetDisabled(buf: number[], id: number, disabled: number): void {
  buf.push(OP.SET_DISABLED);
  buf.push(id);
  buf.push(disabled);
}

export function appendSetSrc(buf: number[], id: number, textIndex: number): void {
  buf.push(OP.SET_SRC);
  buf.push(id);
  buf.push(textIndex);
}

export function appendSetPseudoStyle(buf: number[], id: number, state: number, styleOffset: number, styleLen: number): void {
  buf.push(OP.SET_PSEUDO_STYLE);
  buf.push(id);
  buf.push(state);
  buf.push(styleOffset);
  buf.push(styleLen);
}

export function appendSetStyleVariant(buf: number[], id: number, minWidth: number, maxWidth: number, minHeight: number, maxHeight: number, styleOffset: number, styleLen: number): void {
  buf.push(OP.SET_STYLE_VARIANT);
  buf.push(id);
  buf.push(minWidth);
  buf.push(maxWidth);
  buf.push(minHeight);
  buf.push(maxHeight);
  buf.push(styleOffset);
  buf.push(styleLen);
}

export function appendSetUserSelect(buf: number[], id: number, value: number): void {
  buf.push(OP.SET_USER_SELECT);
  buf.push(id);
  buf.push(value);
}

export function appendSetMultiline(buf: number[], id: number, multiline: number): void {
  buf.push(OP.SET_MULTILINE);
  buf.push(id);
  buf.push(multiline);
}

export function appendSetAriaLabel(buf: number[], id: number, textIndex: number): void {
  buf.push(OP.SET_ARIA_LABEL);
  buf.push(id);
  buf.push(textIndex);
}

export function appendSetRole(buf: number[], id: number, textIndex: number): void {
  buf.push(OP.SET_ROLE);
  buf.push(id);
  buf.push(textIndex);
}

export function appendSetFontFamily(buf: number[], id: number, textIndex: number): void {
  buf.push(OP.SET_FONT_FAMILY);
  buf.push(id);
  buf.push(textIndex);
}

export function appendSetDraw(buf: number[], id: number, drawOffset: number, drawLen: number): void {
  buf.push(OP.SET_DRAW);
  buf.push(id);
  buf.push(drawOffset);
  buf.push(drawLen);
}

export interface DrawPaint {
  readonly color?: readonly [number, number, number, number];
  readonly fillRule?: number;
  readonly strokeWidth?: number;
  readonly cap?: number;
  readonly join?: number;
  readonly miterLimit?: number;
  readonly dash?: readonly number[];
  readonly dashOffset?: number;
}

export function appendDrawMoveTo(draws: number[], x: number, y: number): void {
  draws.push(DRAW_OP.MOVE_TO, x, y);
}

export function appendDrawLineTo(draws: number[], x: number, y: number): void {
  draws.push(DRAW_OP.LINE_TO, x, y);
}

export function appendDrawClose(draws: number[]): void {
  draws.push(DRAW_OP.CLOSE);
}

export function appendDrawFill(draws: number[], paint: DrawPaint): void {
  draws.push(DRAW_OP.FILL);
  const lenIndex = draws.length;
  draws.push(0);
  if (paint.color !== undefined) {
    draws.push(DRAW_PAINT_FIELD.COLOR, ...paint.color);
  }
  if (paint.fillRule !== undefined) {
    draws.push(DRAW_PAINT_FIELD.FILL_RULE, paint.fillRule);
  }
  if (paint.strokeWidth !== undefined) {
    draws.push(DRAW_PAINT_FIELD.STROKE_WIDTH, paint.strokeWidth);
  }
  if (paint.cap !== undefined) {
    draws.push(DRAW_PAINT_FIELD.CAP, paint.cap);
  }
  if (paint.join !== undefined) {
    draws.push(DRAW_PAINT_FIELD.JOIN, paint.join);
  }
  if (paint.miterLimit !== undefined) {
    draws.push(DRAW_PAINT_FIELD.MITER_LIMIT, paint.miterLimit);
  }
  if (paint.dash !== undefined) {
    draws.push(DRAW_PAINT_FIELD.DASH, paint.dash.length, ...paint.dash);
  }
  if (paint.dashOffset !== undefined) {
    draws.push(DRAW_PAINT_FIELD.DASH_OFFSET, paint.dashOffset);
  }
  draws[lenIndex] = draws.length - lenIndex - 1;
}

export function appendDrawQuadraticTo(draws: number[], cx: number, cy: number, x: number, y: number): void {
  draws.push(DRAW_OP.QUADRATIC_TO, cx, cy, x, y);
}

export function appendDrawCubicTo(draws: number[], c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number): void {
  draws.push(DRAW_OP.CUBIC_TO, c1x, c1y, c2x, c2y, x, y);
}

export function appendDrawArcTo(draws: number[], x1: number, y1: number, x2: number, y2: number, radius: number): void {
  draws.push(DRAW_OP.ARC_TO, x1, y1, x2, y2, radius);
}

export function appendDrawRect(draws: number[], x: number, y: number, width: number, height: number): void {
  draws.push(DRAW_OP.RECT, x, y, width, height);
}

export function appendDrawRrect(draws: number[], x: number, y: number, width: number, height: number, rx: number, ry: number): void {
  draws.push(DRAW_OP.RRECT, x, y, width, height, rx, ry);
}

export function appendDrawOval(draws: number[], x: number, y: number, width: number, height: number): void {
  draws.push(DRAW_OP.OVAL, x, y, width, height);
}

export function appendDrawCircle(draws: number[], cx: number, cy: number, radius: number): void {
  draws.push(DRAW_OP.CIRCLE, cx, cy, radius);
}

export function appendDrawStroke(draws: number[], paint: DrawPaint): void {
  draws.push(DRAW_OP.STROKE);
  const lenIndex = draws.length;
  draws.push(0);
  if (paint.color !== undefined) {
    draws.push(DRAW_PAINT_FIELD.COLOR, ...paint.color);
  }
  if (paint.fillRule !== undefined) {
    draws.push(DRAW_PAINT_FIELD.FILL_RULE, paint.fillRule);
  }
  if (paint.strokeWidth !== undefined) {
    draws.push(DRAW_PAINT_FIELD.STROKE_WIDTH, paint.strokeWidth);
  }
  if (paint.cap !== undefined) {
    draws.push(DRAW_PAINT_FIELD.CAP, paint.cap);
  }
  if (paint.join !== undefined) {
    draws.push(DRAW_PAINT_FIELD.JOIN, paint.join);
  }
  if (paint.miterLimit !== undefined) {
    draws.push(DRAW_PAINT_FIELD.MITER_LIMIT, paint.miterLimit);
  }
  if (paint.dash !== undefined) {
    draws.push(DRAW_PAINT_FIELD.DASH, paint.dash.length, ...paint.dash);
  }
  if (paint.dashOffset !== undefined) {
    draws.push(DRAW_PAINT_FIELD.DASH_OFFSET, paint.dashOffset);
  }
  draws[lenIndex] = draws.length - lenIndex - 1;
}

export function appendDrawSave(draws: number[]): void {
  draws.push(DRAW_OP.SAVE);
}

export function appendDrawRestore(draws: number[]): void {
  draws.push(DRAW_OP.RESTORE);
}

export function appendDrawTranslate(draws: number[], dx: number, dy: number): void {
  draws.push(DRAW_OP.TRANSLATE, dx, dy);
}

export function appendDrawRotate(draws: number[], radians: number): void {
  draws.push(DRAW_OP.ROTATE, radians);
}

export function appendDrawScale(draws: number[], sx: number, sy: number): void {
  draws.push(DRAW_OP.SCALE, sx, sy);
}

export function appendDrawTransform(draws: number[], a: number, b: number, c: number, d: number, e: number, f: number): void {
  draws.push(DRAW_OP.TRANSFORM, a, b, c, d, e, f);
}

export function appendDrawClipRect(draws: number[], x: number, y: number, width: number, height: number): void {
  draws.push(DRAW_OP.CLIP_RECT, x, y, width, height);
}

export function appendDrawClipPath(draws: number[]): void {
  draws.push(DRAW_OP.CLIP_PATH);
}
