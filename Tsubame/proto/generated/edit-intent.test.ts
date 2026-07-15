import { describe, expect, it } from 'vitest';
import {
  editDispatchOutcomeFromWire,
  encodeEditIntent,
  type EditDirection,
  type EditGranularity,
} from './edit-intent.js';

describe('generated EditIntent wire capability (#828)', () => {
  it('encodes every payload combination and payload-free variant', () => {
    const granularities: EditGranularity[] = ['grapheme', 'word', 'lineBoundary', 'docBoundary'];
    const directions: EditDirection[] = ['backward', 'forward', 'up', 'down'];
    for (const kind of ['move', 'extend', 'delete'] as const) {
      for (const granularity of granularities) {
        for (const direction of directions) {
          expect(Array.from(encodeEditIntent({ kind, granularity, direction }))).toHaveLength(3);
        }
      }
    }
    for (const kind of ['insertLineBreak', 'selectAll', 'copy', 'cut', 'paste'] as const) {
      expect(Array.from(encodeEditIntent({ kind }))).toHaveLength(1);
    }
  });

  it('decodes all outcomes and rejects unknown outcome codes', () => {
    expect([0, 1, 2].map(editDispatchOutcomeFromWire)).toEqual([
      'consumed',
      'unhandled',
      'deferred',
    ]);
    expect(() => editDispatchOutcomeFromWire(3)).toThrow(RangeError);
  });
});
