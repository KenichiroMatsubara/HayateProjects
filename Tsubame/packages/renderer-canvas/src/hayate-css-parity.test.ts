import { describe, it, expect } from 'vitest';
import type { StylePatch } from '@tsubame/renderer-protocol';
import {
  HAYATE_CSS_CATALOG,
  CATALOG_BY_KEY,
  formatDomCSSValue,
  applyDomExtras,
} from '@tsubame/hayate-css-catalog';
import { encodeStylePatch } from './style-codec.js';
import { TAG } from './protocol.js';

/** Representative sample values per wireKind for semantic parity checks. */
const SAMPLES: Record<string, unknown> = {
  color: '#ff6600',
  dimension: '48px',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'space-between',
  f32: 0.75,
  zIndex: 10,
  fontFamily: 'Inter, sans-serif',
};

function sampleFor(entry: (typeof HAYATE_CSS_CATALOG)[number]): unknown {
  if (entry.patchKey === 'borderWidth') return 2;
  if (entry.patchKey === 'borderRadius' || entry.patchKey === 'fontSize') return 16;
  if (entry.patchKey === 'fontWeight') return 600;
  if (entry.patchKey === 'flexGrow') return 1;
  if (entry.patchKey === 'opacity') return 0.5;
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
      if (entry.patchKey === 'borderWidth') {
        expect(css.borderStyle).toBe('solid');
      }
    }
  });

  it('borderWidth zero sets borderStyle none (dom_extras)', () => {
    const css = domCssForPatch({ borderWidth: 0 });
    expect(css.borderWidth).toBe('0px');
    expect(css.borderStyle).toBe('none');
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
});
