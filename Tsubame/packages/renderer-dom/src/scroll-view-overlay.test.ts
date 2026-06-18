import { describe, it, expect } from 'vitest';
import type { ElementKind } from '@tsubame/renderer-protocol';
import { createDomElement } from './dom-elements.js';

const ALL_KINDS: ElementKind[] = [
  'view',
  'text',
  'image',
  'button',
  'text-input',
  'scroll-view',
];

// Issue #408: the DOM Renderer's `scroll-view` must use *overlay* scrollbars so
// the scrollbar never eats into the content box (no reserved gutter). Canvas has
// no scrollbar gutter concept at all (the wire `overflow` is `visible | hidden`
// only — there is no classic scrollbar to reserve space for), so its scroll-view
// content box is the full padding box. Pinning the DOM scroll-view to overlay is
// what makes the Canvas and DOM `scroll-view` content box widths agree
// (意味論パリティ). Visual canonical stays DOM (ADR-0102), but the gutter must
// not be reserved.
describe('scroll-view overlay scrollbars (issue #408)', () => {
  it('reserves no scrollbar gutter — overlay, not classic UA chrome', () => {
    const el = createDomElement(document, 'scroll-view');
    // `scrollbar-width: none` removes the classic gutter so a scrollable
    // scroll-view keeps the full content box width.
    expect(el.style.scrollbarWidth).toBe('none');
  });

  it('stays scrollable as overlay — overflow still scrolls, gutter not reserved', () => {
    const el = createDomElement(document, 'scroll-view');
    // Overlay must not be bought by killing scrollability (e.g. overflow:hidden):
    // overflow stays a scrolling value so content still scrolls...
    expect(el.style.overflow).toBe('auto');
    // ...and the gutter is never reserved. `scrollbar-gutter: stable` would
    // reserve space and shrink the content box on scroll — the very divergence
    // this issue removes — so it must NOT be set.
    expect(el.style.getPropertyValue('scrollbar-gutter')).not.toBe('stable');
    expect(el.style.scrollbarWidth).toBe('none');
  });

  it('matches Canvas content box width — scroll-view is the only scrollable kind, and it scrolls as overlay', () => {
    // The wire `overflow` vocabulary is `visible | hidden` only — Canvas has no
    // classic scrollbar and so never reserves a gutter; its scroll-view content
    // box is the full padding box. The DOM scroll-view reaches the same content
    // box by scrolling as overlay (no reserved gutter). Other kinds don't scroll,
    // so the gutter question is theirs to begin with: only scroll-view carries
    // the overlay treatment.
    for (const kind of ALL_KINDS) {
      const el = createDomElement(document, kind);
      if (kind === 'scroll-view') {
        expect(el.style.overflow).toBe('auto');
        expect(el.style.scrollbarWidth).toBe('none');
      } else {
        expect(el.style.overflow).toBe('');
      }
    }
  });
});
