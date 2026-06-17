import { describe, it, expect } from 'vitest';
import type { ElementKind } from '@tsubame/renderer-protocol';
import { elementKindDefaultCursor } from '@tsubame/renderer-protocol';
import { createDomElement } from './dom-elements.js';
import { applyStylePatch } from './style-mapping.js';

const ALL_KINDS: ElementKind[] = [
  'view',
  'text',
  'image',
  'button',
  'text-input',
  'scroll-view',
];

describe('createDomElement – RN Web stacking base style', () => {
  it.each(ALL_KINDS)('applies position:relative and zIndex:0 to %s', (kind) => {
    const el = createDomElement(document, kind);
    expect(el.style.position).toBe('relative');
    expect(el.style.zIndex).toBe('0');
  });

  it.each(ALL_KINDS)(
    'applies the spec single-source UA default cursor to %s (ADR-0105)',
    (kind) => {
      const el = createDomElement(document, kind);
      const expected = elementKindDefaultCursor(kind);
      if (expected === undefined) {
        // No kind default — keeps the RN Web base (`inherit`), not a re-declared value.
        expect(el.style.cursor).toBe('inherit');
      } else {
        // button → pointer, text-input → text — from the same table Canvas uses,
        // so DOM and Canvas show the same cursor (Semantics Parity).
        expect(el.style.cursor).toBe(expected);
      }
    },
  );

  it('allows user zIndex to override the base default', () => {
    const el = createDomElement(document, 'view');
    applyStylePatch(el, 'view', { zIndex: 5 });
    expect(el.style.zIndex).toBe('5');
    expect(el.style.position).toBe('relative');
  });
});
