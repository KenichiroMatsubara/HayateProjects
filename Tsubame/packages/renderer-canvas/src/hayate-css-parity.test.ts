import { describe, it, expect } from 'vitest';
import type { StylePatch } from '@tsubame/renderer-protocol';
import {
  HAYATE_CSS_CATALOG,
  CATALOG_BY_KEY,
  formatDomCSSValue,
  applyDomExtras,
} from '@tsubame/hayate-css-catalog';
import { encodeStylePatch } from '@tsubame/protocol-generated/codec';
import { TAG } from '@tsubame/protocol-generated/protocol';

/** Representative sample values per wireKind for semantic parity checks. */
const SAMPLES: Record<string, unknown> = {
  color: '#ff6600',
  dimension: '48px',
  dimensionList: ['100px', '1fr', '50%'],
  display: 'flex',
  flexDirection: 'column',
  flexWrap: 'wrap',
  alignItems: 'center',
  alignSelf: 'flex-end',
  alignContent: 'space-between',
  justifyContent: 'space-between',
  fontStyle: 'italic',
  textDecoration: 'underline',
  borderStyle: 'dashed',
  f32: 0.75,
  zIndex: 10,
  fontFamily: 'Inter, sans-serif',
};

function sampleFor(entry: (typeof HAYATE_CSS_CATALOG)[number]): unknown {
  if (entry.patchKey === 'borderWidth') return 2;
  if (entry.patchKey === 'borderRadius' || entry.patchKey === 'fontSize') return 16;
  if (entry.patchKey === 'fontWeight') return 600;
  if (entry.patchKey === 'flexGrow') return 1;
  if (entry.patchKey === 'flexShrink') return 0.5;
  if (entry.patchKey === 'flexBasis') return '80px';
  if (entry.patchKey === 'opacity') return 0.5;
  if (entry.patchKey === 'defaultFontSize') return 16;
  if (entry.patchKey === 'defaultFontWeight') return 600;
  return SAMPLES[entry.wireKind];
}

function domCssForPatch(patch: StylePatch): Record<string, string> {
  const style: Record<string, string> = {};
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined || value === null) continue;
    const entry = CATALOG_BY_KEY[k as string]!;
    style[entry.cssName] = formatDomCSSValue(entry, value);
    applyDomExtras(style, entry, value);
  }
  return style;
}

describe('hayate-css catalog parity', () => {
  it('sampleFor provides a defined value for every catalog entry', () => {
    for (const entry of HAYATE_CSS_CATALOG) {
      expect(sampleFor(entry), entry.patchKey).toBeDefined();
    }
  });

  it('covers every catalog entry with packet and css targets', () => {
    expect(HAYATE_CSS_CATALOG.length).toBeGreaterThan(0);
    for (const entry of HAYATE_CSS_CATALOG) {
      expect(entry.targets).toContain('packet');
      expect(entry.targets).toContain('css');
      expect(CATALOG_BY_KEY[entry.patchKey]).toBe(entry);
    }
  });

  it('encodeStylePatch tag matches catalog tag for each entry', () => {
    for (const entry of HAYATE_CSS_CATALOG) {
      const sample = sampleFor(entry);
      const patch = { [entry.patchKey]: sample } as StylePatch;
      const out: number[] = [];
      encodeStylePatch(patch, out);
      expect(out[0]).toBe(entry.tag);
      expect(out.length).toBeGreaterThan(1);
    }
  });

  it('DOM css string is produced for each catalog entry sample', () => {
    for (const entry of HAYATE_CSS_CATALOG) {
      const sample = sampleFor(entry);
      const patch = { [entry.patchKey]: sample } as StylePatch;
      const css = domCssForPatch(patch);
      expect(css[entry.cssName]).toBeTruthy();
    }
  });

  it('borderStyle maps directly to CSS border-style (no width coupling, #204)', () => {
    expect(domCssForPatch({ borderStyle: 'dashed' }).borderStyle).toBe('dashed');
    expect(domCssForPatch({ borderStyle: 'none' }).borderStyle).toBe('none');
    // border-width no longer emits a border-style of its own.
    expect(domCssForPatch({ borderWidth: 2 }).borderStyle).toBeUndefined();
  });

  it('flexbox completion properties produce expected DOM CSS strings', () => {
    expect(domCssForPatch({ flexShrink: 0.5 }).flexShrink).toBe('0.5');
    expect(domCssForPatch({ flexBasis: '80px' }).flexBasis).toBe('80px');
    expect(domCssForPatch({ alignSelf: 'flex-end' }).alignSelf).toBe('flex-end');
    expect(domCssForPatch({ alignContent: 'space-between' }).alignContent).toBe('space-between');
    expect(domCssForPatch({ flexWrap: 'wrap-reverse' }).flexWrap).toBe('wrap-reverse');
  });

  it('dimension encode and DOM css both use px for numeric values', () => {
    const patch = { width: 100 } as StylePatch;
    const out: number[] = [];
    encodeStylePatch(patch, out);
    expect(out[0]).toBe(TAG.WIDTH);
    expect(out[1]).toBe(100);
    expect(out[2]).toBe(0);
    expect(domCssForPatch(patch).width).toBe('100px');
  });

  it('ambient default* tags map to inheritable CSS properties (ADR-0070)', () => {
    const ambient = [
      ['defaultColor', 'color', 'color'],
      ['defaultFontFamily', 'fontFamily', 'font-family'],
      ['defaultFontSize', 'fontSize', 'font-size'],
      ['defaultFontWeight', 'fontWeight', 'font-weight'],
    ] as const;
    for (const [patchKey, cssName, cssProperty] of ambient) {
      const entry = CATALOG_BY_KEY[patchKey]!;
      expect(entry.cssProperty).toBe(cssProperty);
      expect(entry.cssName).toBe(cssName);
    }
    expect(CATALOG_BY_KEY.defaultFontWeight!.domFormat).toBe('number');
    expect(domCssForPatch({ defaultFontWeight: 600 }).fontWeight).toBe('600');
  });
});
