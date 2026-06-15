import { describe, it, expect, beforeEach } from 'vitest';
import type { ElementKind } from '@tsubame/renderer-protocol';
import { withTextLocalGate, carriesTextLocal } from '@tsubame/renderer-protocol';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

// Structure-based Semantics Parity (Tsubame ADR-0008, #323). The Style Channel
// gate no longer lives inside each renderer — it runs once in the seam
// (`withTextLocalGate`) before any renderer. So instead of comparing two
// emitters, this drives the real DOM renderer *through the production seam* and
// checks that a channel-1 text-local prop reaches the element iff the kind
// carries text-local. Parity with Canvas (and any future renderer) is then
// structural: they all receive the identical gated patch.

const ALL_KINDS: readonly ElementKind[] = [
  'view',
  'text',
  'image',
  'button',
  'text-input',
  'scroll-view',
];

describe('text-local gate through the seam (DOM renderer, Tsubame ADR-0008, #323)', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  for (const kind of ALL_KINDS) {
    it(`${kind}: keeps text-local color iff the kind carries text-local`, () => {
      const renderer = withTextLocalGate(new DomRenderer({ document, container }));
      const id = renderer.createElement(kind);
      renderer.setRoot(id);
      renderer.setStyle(id, { color: '#ff0000', width: '100px' });

      const el = container.querySelector(`[data-tsubame-id="${id as number}"]`) as HTMLElement;
      // A non-text-local prop always applies.
      expect(el.style.width).toBe('100px');
      // A text-local prop applies only on a Text-Local Carrier kind.
      if (carriesTextLocal(kind)) {
        expect(el.style.color).toBe('#ff0000');
      } else {
        expect(el.style.color).toBe('');
      }
    });
  }
});
