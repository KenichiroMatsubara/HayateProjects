import type {
  AlignItems,
  Display,
  ElementKind,
  FlexDirection,
  HayateDimension,
  JustifyContent,
  StylePatch,
} from '@tsubame/renderer-protocol';

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

export type HayateStyleProp =
  | { 'background-color': HayateColorRecord }
  | { opacity: number }
  | { 'border-radius': number }
  | { 'border-width': number }
  | { 'border-color': HayateColorRecord }
  | { width: HayateDimensionRecord }
  | { height: HayateDimensionRecord }
  | { 'min-width': HayateDimensionRecord }
  | { 'min-height': HayateDimensionRecord }
  | { 'max-width': HayateDimensionRecord }
  | { 'max-height': HayateDimensionRecord }
  | { display: Display }
  | { 'flex-direction': FlexDirection }
  | { 'align-items': AlignItems }
  | { 'justify-content': JustifyContent }
  | { gap: HayateDimensionRecord }
  | { padding: HayateDimensionRecord }
  | { 'padding-top': HayateDimensionRecord }
  | { 'padding-right': HayateDimensionRecord }
  | { 'padding-bottom': HayateDimensionRecord }
  | { 'padding-left': HayateDimensionRecord }
  | { margin: HayateDimensionRecord }
  | { 'margin-top': HayateDimensionRecord }
  | { 'margin-right': HayateDimensionRecord }
  | { 'margin-bottom': HayateDimensionRecord }
  | { 'margin-left': HayateDimensionRecord }
  | { 'font-size': number }
  | { 'font-family': string }
  | { color: HayateColorRecord }
  | { 'z-index': number }
  | { 'flex-grow': number };

export type HayateStylePropKind = 'color' | 'font-size' | 'font-family';

export type HayateEvent =
  | { type: 'click'; target: number; x: number; y: number }
  | { type: 'hover-enter'; target: number }
  | { type: 'hover-leave'; target: number }
  | { type: 'active-start'; target: number }
  | { type: 'active-end'; target: number }
  | { type: 'pointer-move'; x: number; y: number }
  | { type: 'key-down'; target: number; key: string; modifiers: number }
  | { type: 'focus'; target: number }
  | { type: 'blur'; target: number }
  | { type: 'text-input'; target: number; text: string }
  | { type: 'composition-start'; target: number; text: string }
  | { type: 'composition-update'; target: number; text: string }
  | { type: 'composition-end'; target: number; text: string }
  | { type: 'scroll'; target: number; 'delta-x': number; 'delta-y': number }
  | { type: 'resize'; width: number; height: number };

export interface HayateWasm {
  element_create(id: number, kind: ElementKind): void;
  set_root(id: number): void;
  element_append_child(parent: number, child: number): void;
  element_insert_before(parent: number, child: number, before: number): void;
  element_remove(id: number): void;
  element_set_style(id: number, props: HayateStyleProp[]): void;
  element_unset_style(id: number, kinds: HayateStylePropKind[]): void;
  element_set_text(id: number, text: string): void;
  on_resize(width: number, height: number): void;
  render(timestampMs: number): void;
  poll_events(): HayateEvent[];
}

export function stylePatchToMutation(patch: StylePatch): {
  props: HayateStyleProp[];
  unsetKinds: HayateStylePropKind[];
} {
  const props: HayateStyleProp[] = [];
  const unsetKinds: HayateStylePropKind[] = [];

  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined) continue;

    if (value === null) {
      switch (k) {
        case 'color':
          unsetKinds.push('color');
          break;
        case 'fontSize':
          unsetKinds.push('font-size');
          break;
        case 'fontFamily':
          unsetKinds.push('font-family');
          break;
        default:
          throw new Error(`CanvasRenderer: WIT does not support unsetting "${k}"`);
      }
      continue;
    }

    switch (k) {
      case 'backgroundColor':
        props.push({ 'background-color': parseColor(value as string) });
        break;
      case 'opacity':
        props.push({ opacity: finiteNumber(k, value) });
        break;
      case 'borderRadius':
        props.push({ 'border-radius': finiteNumber(k, value) });
        break;
      case 'borderWidth':
        props.push({ 'border-width': finiteNumber(k, value) });
        break;
      case 'borderColor':
        props.push({ 'border-color': parseColor(value as string) });
        break;
      case 'width':
        props.push({ width: parseDimension(value as HayateDimension) });
        break;
      case 'height':
        props.push({ height: parseDimension(value as HayateDimension) });
        break;
      case 'minWidth':
        props.push({ 'min-width': parseDimension(value as HayateDimension) });
        break;
      case 'minHeight':
        props.push({ 'min-height': parseDimension(value as HayateDimension) });
        break;
      case 'maxWidth':
        props.push({ 'max-width': parseDimension(value as HayateDimension) });
        break;
      case 'maxHeight':
        props.push({ 'max-height': parseDimension(value as HayateDimension) });
        break;
      case 'display':
        props.push({ display: value as Display });
        break;
      case 'flexDirection':
        props.push({ 'flex-direction': value as FlexDirection });
        break;
      case 'alignItems':
        props.push({ 'align-items': value as AlignItems });
        break;
      case 'justifyContent':
        props.push({ 'justify-content': value as JustifyContent });
        break;
      case 'gap':
        props.push({ gap: parseDimension(value as HayateDimension) });
        break;
      case 'padding':
        props.push({ padding: parseDimension(value as HayateDimension) });
        break;
      case 'paddingTop':
        props.push({ 'padding-top': parseDimension(value as HayateDimension) });
        break;
      case 'paddingRight':
        props.push({ 'padding-right': parseDimension(value as HayateDimension) });
        break;
      case 'paddingBottom':
        props.push({ 'padding-bottom': parseDimension(value as HayateDimension) });
        break;
      case 'paddingLeft':
        props.push({ 'padding-left': parseDimension(value as HayateDimension) });
        break;
      case 'margin':
        props.push({ margin: parseDimension(value as HayateDimension) });
        break;
      case 'marginTop':
        props.push({ 'margin-top': parseDimension(value as HayateDimension) });
        break;
      case 'marginRight':
        props.push({ 'margin-right': parseDimension(value as HayateDimension) });
        break;
      case 'marginBottom':
        props.push({ 'margin-bottom': parseDimension(value as HayateDimension) });
        break;
      case 'marginLeft':
        props.push({ 'margin-left': parseDimension(value as HayateDimension) });
        break;
      case 'fontSize':
        props.push({ 'font-size': finiteNumber(k, value) });
        break;
      case 'fontFamily':
        props.push({ 'font-family': String(value) });
        break;
      case 'color':
        props.push({ color: parseColor(value as string) });
        break;
      case 'zIndex':
        props.push({ 'z-index': finiteInteger(k, value) });
        break;
      case 'flexGrow':
        props.push({ 'flex-grow': finiteNumber(k, value) });
        break;
      case 'fontWeight':
        throw new Error('CanvasRenderer: "fontWeight" is not defined in WIT');
      default:
        throw new Error(`CanvasRenderer: unsupported WIT style property "${k}"`);
    }
  }

  return { props, unsetKinds };
}

function finiteNumber(key: string, value: unknown): number {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    throw new Error(`CanvasRenderer: invalid numeric value for "${key}"`);
  }
  return numeric;
}

function finiteInteger(key: string, value: unknown): number {
  const numeric = finiteNumber(key, value);
  if (!Number.isInteger(numeric)) {
    throw new Error(`CanvasRenderer: "${key}" must be an integer`);
  }
  return numeric;
}

function parseDimension(value: HayateDimension): HayateDimensionRecord {
  if (typeof value === 'number') {
    return { value, unit: 'px' };
  }

  const trimmed = value.trim().toLowerCase();
  if (trimmed === 'auto') {
    return { value: 0, unit: 'auto' };
  }

  const match = trimmed.match(/^(-?(?:\d+|\d*\.\d+))(px|%|fr)?$/);
  if (match === null) {
    throw new Error(`CanvasRenderer: unsupported WIT dimension "${value}"`);
  }

  const numeric = Number(match[1]);
  if (!Number.isFinite(numeric)) {
    throw new Error(`CanvasRenderer: invalid WIT dimension "${value}"`);
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

  throw new Error(`CanvasRenderer: unsupported WIT color "${input}"`);
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
