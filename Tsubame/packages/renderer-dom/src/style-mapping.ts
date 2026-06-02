import type { HayateDimension, HayateStyle, StylePatch } from '@tsubame/renderer-protocol';

const CSS_PROP: Record<keyof HayateStyle, string> = {
  width: 'width',
  height: 'height',
  minWidth: 'minWidth',
  minHeight: 'minHeight',
  maxWidth: 'maxWidth',
  maxHeight: 'maxHeight',
  display: 'display',
  flexDirection: 'flexDirection',
  alignItems: 'alignItems',
  justifyContent: 'justifyContent',
  gap: 'gap',
  flexGrow: 'flexGrow',
  padding: 'padding',
  paddingTop: 'paddingTop',
  paddingRight: 'paddingRight',
  paddingBottom: 'paddingBottom',
  paddingLeft: 'paddingLeft',
  margin: 'margin',
  marginTop: 'marginTop',
  marginRight: 'marginRight',
  marginBottom: 'marginBottom',
  marginLeft: 'marginLeft',
  color: 'color',
  backgroundColor: 'backgroundColor',
  borderColor: 'borderColor',
  borderRadius: 'borderRadius',
  borderWidth: 'borderWidth',
  opacity: 'opacity',
  zIndex: 'zIndex',
  fontSize: 'fontSize',
  fontFamily: 'fontFamily',
  fontWeight: 'fontWeight',
};

const DIM_PROPS = new Set<keyof HayateStyle>([
  'width',
  'height',
  'minWidth',
  'minHeight',
  'maxWidth',
  'maxHeight',
  'gap',
  'padding',
  'paddingTop',
  'paddingRight',
  'paddingBottom',
  'paddingLeft',
  'margin',
  'marginTop',
  'marginRight',
  'marginBottom',
  'marginLeft',
]);

const PX_NUMBER_PROPS = new Set<keyof HayateStyle>([
  'borderRadius',
  'borderWidth',
  'fontSize',
]);

function formatDimension(value: HayateDimension): string {
  return typeof value === 'number' ? `${value}px` : value;
}

function format(key: keyof HayateStyle, value: NonNullable<unknown>): string {
  if (DIM_PROPS.has(key)) return formatDimension(value as HayateDimension);
  if (PX_NUMBER_PROPS.has(key) && typeof value === 'number') return `${value}px`;
  return String(value);
}

export function applyStylePatch(el: HTMLElement, patch: StylePatch): void {
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined) continue;

    const cssProp = CSS_PROP[k];
    if (cssProp === undefined) {
      throw new Error(`DOMRenderer: unknown Hayate style property "${k}"`);
    }

    const style = el.style as unknown as Record<string, string>;
    style[cssProp] = value === null ? '' : format(k, value);
    if (k === 'borderWidth' && value !== null) {
      el.style.borderStyle = Number(value) > 0 ? 'solid' : 'none';
    }
  }
}
