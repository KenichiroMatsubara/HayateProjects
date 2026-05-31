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
 * Hayate の `style_packet.rs` の TAG 定数と 1:1 対応。
 * OP_SET_STYLE レコードが指す区間に、プロパティごとのエントリを連結する。
 *
 * エントリのレイアウト（op フィールドなし）:
 *   スカラー:  [tag, value]              (例: opacity, borderRadius, fontSize)
 *   カラー:    [tag, r, g, b, a]         (0.0–1.0 各成分)
 *   ディメンジョン: [tag, value, unit_code] (unit_code=0 固定で px 単位)
 *   列挙:      [tag, code]               (display, flexDirection, …)
 *
 * null 値（リセット）は Rust が未対応のためスキップする。
 * FONT_WEIGHT は Rust `StyleProp` に未実装のため Canvas では無視する。
 */
export const TAG = {
  BACKGROUND_COLOR:  0,
  OPACITY:           1,
  BORDER_RADIUS:     2,
  // 3=BORDER_WIDTH, 4=BORDER_COLOR は HayateStyle に未定義のためスキップ
  WIDTH:             5,
  HEIGHT:            6,
  // 7=MIN_WIDTH, 8=MIN_HEIGHT, 9=MAX_WIDTH, 10=MAX_HEIGHT は HayateStyle 未定義
  DISPLAY:          11,
  FLEX_DIRECTION:   12,
  ALIGN_ITEMS:      13,
  JUSTIFY_CONTENT:  14,
  GAP:              15,
  // 16–25 = padding/margin 系（HayateStyle 未定義）
  FONT_SIZE:        26,
  COLOR:            27,
  // 28=Z_INDEX, 29=FONT_FAMILY は HayateStyle 未定義
  FLEX_GROW:        30,
} as const;

// ─── 列挙コード（Rust の AlignValue / DisplayValue / … と一致）────────────

const DISPLAY_CODE: Record<Display, number> = { flex: 0, none: 3 };

const FLEX_DIRECTION_CODE: Record<FlexDirection, number> = { row: 0, column: 1 };

const ALIGN_ITEMS_CODE: Record<AlignItems, number> = {
  'flex-start': 0,
  'flex-end':   1,
  center:       2,
  stretch:      3,
};

const JUSTIFY_CONTENT_CODE: Record<JustifyContent, number> = {
  'flex-start':   0,
  'flex-end':     1,
  center:         2,
  'space-between': 3,
  'space-around':  4,
  'space-evenly':  5,
};

// ─── ディメンジョン系プロパティ（Rust が [value, unit_raw] 2 slots を要求）──

const DIM_KEYS = new Set<keyof HayateStyle>(['width', 'height', 'gap']);

/** `'100%'` のような CSS パーセント文字列を [value, unit_code] にパース。 */
function parseDimension(value: number | string): [number, number] {
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (trimmed.endsWith('%')) {
      return [parseFloat(trimmed), 1]; // unit_code=1 → Percent
    }
    return [parseFloat(trimmed), 0]; // fallback: px
  }
  return [value, 0]; // unit_code=0 → Px
}

// ─── カラー系プロパティ（[r, g, b, a] 4 slots）──────────────────────────────

const COLOR_KEYS = new Set<keyof HayateStyle>(['color', 'backgroundColor']);

// ─── TAG マッピング（FONT_WEIGHT は Rust 未対応のため含めない）──────────────

const TAG_BY_KEY: Partial<Record<keyof HayateStyle, number>> = {
  width:           TAG.WIDTH,
  height:          TAG.HEIGHT,
  display:         TAG.DISPLAY,
  flexDirection:   TAG.FLEX_DIRECTION,
  alignItems:      TAG.ALIGN_ITEMS,
  justifyContent:  TAG.JUSTIFY_CONTENT,
  gap:             TAG.GAP,
  flexGrow:        TAG.FLEX_GROW,
  color:           TAG.COLOR,
  backgroundColor: TAG.BACKGROUND_COLOR,
  borderRadius:    TAG.BORDER_RADIUS,
  opacity:         TAG.OPACITY,
  fontSize:        TAG.FONT_SIZE,
  // fontWeight: 意図的に除外（Rust StyleProp に未実装）
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
    if (hex.length === 8) return [read2(0), read2(2), read2(4), read2(6)];
  }
  const m = s.match(/rgba?\(([^)]+)\)/);
  if (m) {
    const parts = m[1]!.split(',').map((p) => parseFloat(p.trim()));
    const [r = 0, g = 0, b = 0, a = 1] = parts;
    return [r / 255, g / 255, b / 255, a];
  }
  return [0, 0, 0, 1];
}

function enumCodeFor(key: keyof HayateStyle, value: NonNullable<unknown>): number {
  switch (key) {
    case 'display':         return DISPLAY_CODE[value as Display];
    case 'flexDirection':   return FLEX_DIRECTION_CODE[value as FlexDirection];
    case 'alignItems':      return ALIGN_ITEMS_CODE[value as AlignItems];
    case 'justifyContent':  return JUSTIFY_CONTENT_CODE[value as JustifyContent];
    default: return value as number;
  }
}

/**
 * {@link StylePatch} を styles バッファへ書き込む。
 *
 * - null 値はスキップ（Rust 側に reset 機構がないため）
 * - fontWeight は Rust 未対応のためスキップ
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
    // null（リセット）と未対応プロパティはスキップ
    if (value == null) continue;
    const tag = TAG_BY_KEY[k];
    if (tag === undefined) continue;

    buffer[cursor++] = tag;

    if (COLOR_KEYS.has(k)) {
      const [r, g, b, a] = parseColor(value as string);
      buffer[cursor++] = r;
      buffer[cursor++] = g;
      buffer[cursor++] = b;
      buffer[cursor++] = a;
    } else if (DIM_KEYS.has(k)) {
      const [dimVal, unitCode] = parseDimension(value as number | string);
      buffer[cursor++] = dimVal;
      buffer[cursor++] = unitCode;
    } else if (
      k === 'display' ||
      k === 'flexDirection' ||
      k === 'alignItems' ||
      k === 'justifyContent'
    ) {
      buffer[cursor++] = enumCodeFor(k, value);
    } else {
      // スカラー（opacity, borderRadius, fontSize, …）
      buffer[cursor++] = value as number;
    }
  }
  return cursor - offset;
}
