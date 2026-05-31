import type {
  HayateStyle,
  StylePatch,
  Display,
  FlexDirection,
  AlignItems,
  JustifyContent,
} from '@tsubame/renderer-protocol';

/**
 * styles バッファ（flat Float32Array）の TAG エンコーディング。
 *
 * Hayate の `style_packet.rs` の TAG エンコーディングに対応する Tsubame 側の
 * エンコーダ。OP_SET_STYLE の `style_offset` / `style_len` が指す区間に、
 * プロパティごとのエントリを連結して書き込む。
 *
 * 各エントリのレイアウト:
 *   `[tag, op, ...values]`
 *   - `op`: 1 = set（値あり）/ 0 = reset（値なし、デフォルトへ戻す）
 *   - values の個数は tag の arity で決まる（スカラー=1, color=4[r,g,b,a]）
 *
 * Hayate 側パーサーは tag から arity を判定し、self-describing に区間を走査する。
 */
export const TAG = {
  WIDTH: 1,
  HEIGHT: 2,
  DISPLAY: 3,
  FLEX_DIRECTION: 4,
  ALIGN_ITEMS: 5,
  JUSTIFY_CONTENT: 6,
  GAP: 7,
  COLOR: 8,
  BACKGROUND_COLOR: 9,
  BORDER_RADIUS: 10,
  OPACITY: 11,
  FONT_SIZE: 12,
  FONT_WEIGHT: 13,
} as const;

const OP_RESET = 0;
const OP_SET = 1;

const DISPLAY_CODE: Record<Display, number> = { flex: 0, none: 1 };
const FLEX_DIRECTION_CODE: Record<FlexDirection, number> = { row: 0, column: 1 };
const ALIGN_ITEMS_CODE: Record<AlignItems, number> = {
  'flex-start': 0,
  'flex-end': 1,
  center: 2,
  stretch: 3,
};
const JUSTIFY_CONTENT_CODE: Record<JustifyContent, number> = {
  'flex-start': 0,
  'flex-end': 1,
  center: 2,
  'space-between': 3,
  'space-around': 4,
  'space-evenly': 5,
};

/** color プロパティ（4 値 [r,g,b,a]、0..1）か否か。 */
const COLOR_TAGS = new Set<number>([TAG.COLOR, TAG.BACKGROUND_COLOR]);

const TAG_BY_KEY: Record<keyof HayateStyle, number> = {
  width: TAG.WIDTH,
  height: TAG.HEIGHT,
  display: TAG.DISPLAY,
  flexDirection: TAG.FLEX_DIRECTION,
  alignItems: TAG.ALIGN_ITEMS,
  justifyContent: TAG.JUSTIFY_CONTENT,
  gap: TAG.GAP,
  color: TAG.COLOR,
  backgroundColor: TAG.BACKGROUND_COLOR,
  borderRadius: TAG.BORDER_RADIUS,
  opacity: TAG.OPACITY,
  fontSize: TAG.FONT_SIZE,
  fontWeight: TAG.FONT_WEIGHT,
};

/** [r,g,b,a]（各 0..1）。未解釈の色は黒・不透明にフォールバック。 */
export function parseColor(input: string): [number, number, number, number] {
  const s = input.trim().toLowerCase();
  if (s.startsWith('#')) {
    const hex = s.slice(1);
    const read = (i: number): number => parseInt(hex[i]! + hex[i]!, 16) / 255;
    const read2 = (i: number): number =>
      parseInt(hex.slice(i, i + 2), 16) / 255;
    if (hex.length === 3) return [read(0), read(1), read(2), 1];
    if (hex.length === 4) return [read(0), read(1), read(2), read(3)];
    if (hex.length === 6) return [read2(0), read2(2), read2(4), 1];
    if (hex.length === 8)
      return [read2(0), read2(2), read2(4), read2(6)];
  }
  const m = s.match(/rgba?\(([^)]+)\)/);
  if (m) {
    const parts = m[1]!.split(',').map((p) => parseFloat(p.trim()));
    const [r = 0, g = 0, b = 0, a = 1] = parts;
    return [r / 255, g / 255, b / 255, a];
  }
  return [0, 0, 0, 1];
}

function valueFor(key: keyof HayateStyle, value: NonNullable<unknown>): number {
  switch (key) {
    case 'display':
      return DISPLAY_CODE[value as Display];
    case 'flexDirection':
      return FLEX_DIRECTION_CODE[value as FlexDirection];
    case 'alignItems':
      return ALIGN_ITEMS_CODE[value as AlignItems];
    case 'justifyContent':
      return JUSTIFY_CONTENT_CODE[value as JustifyContent];
    default:
      return value as number; // width/height/gap/borderRadius/opacity/fontSize/fontWeight
  }
}

/**
 * {@link StylePatch} を styles バッファへ書き込む。
 *
 * @returns 書き込んだ f32 slot 数（OP_SET_STYLE の style_len）。
 */
export function encodeStylePatch(
  patch: StylePatch,
  buffer: Float32Array,
  offset: number,
): number {
  let cursor = offset;
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined) continue;
    const tag = TAG_BY_KEY[k];
    buffer[cursor++] = tag;
    if (value === null) {
      buffer[cursor++] = OP_RESET;
      continue;
    }
    buffer[cursor++] = OP_SET;
    if (COLOR_TAGS.has(tag)) {
      const [r, g, b, a] = parseColor(value as string);
      buffer[cursor++] = r;
      buffer[cursor++] = g;
      buffer[cursor++] = b;
      buffer[cursor++] = a;
    } else {
      buffer[cursor++] = valueFor(k, value);
    }
  }
  return cursor - offset;
}
