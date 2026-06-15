import { describe, it, expect } from 'vitest';
import type { ElementKind, StylePatch } from '@tsubame/renderer-protocol';
import { gateTextLocalPatch } from '@tsubame/renderer-protocol';
import { declarationsFromStylePatch } from './style-declarations.js';

// Semantics Parity harness (Tsubame ADR-0008, #305): the Style Channel gate must
// produce the same result on both renderer paths for the same (kind, patch). The
// DOM path emits CSS declarations (`declarationsFromStylePatch`); the Canvas path
// filters the patch before encode (`gateTextLocalPatch`, the function the Canvas
// renderer calls). For a given prop on a given kind, the DOM keeps it iff the
// Canvas keeps it — anything else is the silent divergence this issue closes.

const ALL_KINDS: readonly ElementKind[] = [
  'view',
  'text',
  'image',
  'button',
  'text-input',
  'scroll-view',
];

// Channel-1 text-local props (gated) plus non-text-local controls (never gated),
// each with a value the DOM catalog can format into a declaration.
const PROPS: ReadonlyArray<readonly [keyof StylePatch, unknown]> = [
  ['color', '#ff0000'],
  ['fontSize', 16],
  ['fontWeight', 600],
  ['fontStyle', 'italic'],
  ['textDecoration', 'underline'],
  ['fontFamily', 'Arial'],
  ['backgroundColor', '#00ff00'],
  ['width', '10px'],
];

/** DOM path: does this single-prop patch survive into a CSS declaration? */
function domApplies(kind: ElementKind, key: keyof StylePatch, value: unknown): boolean {
  const patch = { [key]: value } as StylePatch;
  return declarationsFromStylePatch(kind, patch, { onUnknownKey: 'skip' }).length > 0;
}

/** Canvas path: does this prop survive the pre-encode gate? */
function canvasKeeps(kind: ElementKind, key: keyof StylePatch, value: unknown): boolean {
  const patch = { [key]: value } as StylePatch;
  return key in gateTextLocalPatch(kind, patch);
}

describe('text-local gate parity (DOM vs Canvas, Tsubame ADR-0008, #305)', () => {
  for (const kind of ALL_KINDS) {
    for (const [key, value] of PROPS) {
      it(`${kind} / ${String(key)}: DOM and Canvas agree on the gate`, () => {
        expect(canvasKeeps(kind, key, value)).toBe(domApplies(kind, key, value));
      });
    }
  }

  it('a mixed patch keeps the same prop set on both paths', () => {
    const patch: StylePatch = {
      color: '#ff0000',
      fontSize: 16,
      backgroundColor: '#00ff00',
      width: '10px',
    };
    for (const kind of ALL_KINDS) {
      const canvasKept = Object.keys(gateTextLocalPatch(kind, patch)).sort();
      const domKept = (Object.keys(patch) as (keyof StylePatch)[])
        .filter((key) => domApplies(kind, key, patch[key]))
        .sort();
      expect(canvasKept).toEqual(domKept);
    }
  });
});
