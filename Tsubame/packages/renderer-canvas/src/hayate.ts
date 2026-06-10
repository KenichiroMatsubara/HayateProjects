import type { HayateDimension } from '@tsubame/renderer-protocol';

/**
 * 実 Hayate WASM（`HayateElementRenderer`）が公開する WIT element-layer の
 * メソッドのうち、`CanvasRenderer` が呼び出すものだけを型付けした最小 interface。
 *
 * wasm-bindgen 生成クラスは構造的にこれを充足するため、`init.ts` では
 * 生成インスタンスをそのまま `RawHayate` として渡せる。スタイルは
 * `Float32Array`（style-packet TAG 形式）、ミューテーションは
 * `apply_mutations(ops, styles, texts)`（ADR-0052）、イベントは delivery
 * poll（ADR-0053: `[listener_id, kind, ...fields]`）でやり取りする。
 */
export interface RawHayate {
  element_create(id: number, kind: number): void;
  set_root(id: number): void;
  element_append_child(parent: number, child: number): void;
  element_insert_before(parent: number, child: number, before: number): void;
  element_remove(id: number): void;
  element_set_text(id: number, text: string): void;
  element_set_text_content(id: number, text: string): void;
  element_set_src(id: number, url: string): void;
  element_set_disabled(id: number, disabled: boolean): void;
  element_get_text(id: number): string;
  /** Element ids in `id` and its descendants (Hayate ElementTree is authoritative). */
  element_subtree_ids(id: number): Float64Array;
  element_set_style(id: number, packed: Float32Array): void;
  element_set_pseudo_style(id: number, state: number, packed: Float32Array): void;
  apply_mutations(
    ops: Float64Array,
    styles: Float32Array,
    texts: string[],
  ): void;
  on_resize(width: number, height: number): void;
  on_pointer_move(x: number, y: number): void;
  on_pointer_down(x: number, y: number): void;
  on_pointer_up(x: number, y: number): void;
  on_wheel(x: number, y: number, deltaX: number, deltaY: number): void;
  on_key_down(key: string, modifiers: number): void;
  on_text_input(id: number, text: string): void;
  on_composition_start(id: number, text: string): void;
  on_composition_update(id: number, text: string): void;
  on_composition_end(id: number, text: string): void;
  focused_element_id(): number;
  /** Cursor rect synced during the last render (ADR-0069). */
  ime_character_bounds(): number[];
  render(timestampMs: number): void;
  poll_events(): unknown[];
  register_listener(element_id: number, event_kind: number): number;
  set_background_color(r: number, g: number, b: number): void;
  /** Resolved style after inheritance + pseudo-state (ADR-0067), or `null` if `id` is unknown. */
  element_effective_visual(id: number): HayateEffectiveVisual | null;
}

/** JS-friendly mirror of `hayate_core::Visual` after effective-style resolution (ADR-0067). */
export interface HayateEffectiveVisual {
  backgroundColor: HayateColorRecord | null;
  opacity: number;
  borderRadius: number;
  borderWidth: number;
  borderColor: HayateColorRecord | null;
  textColor: HayateColorRecord | null;
  fontSize: number | null;
  fontWeight: number | null;
  fontStyle: 'normal' | 'italic' | 'oblique' | null;
  textDecoration: 'none' | 'underline' | 'line-through' | null;
  zIndex: number;
  fontFamily: string | null;
}

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
    throw new Error(`CanvasRenderer: invalid numeric value for "${key}"`);
  }
  return numeric;
}

export function finiteInteger(key: string, value: unknown): number {
  const numeric = finiteNumber(key, value);
  if (!Number.isInteger(numeric)) {
    throw new Error(`CanvasRenderer: "${key}" must be an integer`);
  }
  return numeric;
}

export function parseDimension(value: HayateDimension): HayateDimensionRecord {
  if (typeof value === 'number') {
    return { value, unit: 'px' };
  }

  const trimmed = value.trim().toLowerCase();
  if (trimmed === 'auto') {
    return { value: 0, unit: 'auto' };
  }

  const match = trimmed.match(/^(-?(?:\d+|\d*\.\d+))(px|%|fr)?$/);
  if (match === null) {
    throw new Error(`CanvasRenderer: unsupported dimension "${value}"`);
  }

  const numeric = Number(match[1]);
  if (!Number.isFinite(numeric)) {
    throw new Error(`CanvasRenderer: invalid dimension "${value}"`);
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

  throw new Error(`CanvasRenderer: unsupported color "${input}"`);
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
