import { describe, it, expect } from 'vitest';
import { OP, TAG, ELEMENT_KIND } from '@tsubame/protocol-generated/protocol';
import { UNSET_KIND } from '@tsubame/protocol-generated/protocol';
import {
  encodeMutations,
  splitStyleVariant,
  viewportAxis,
  type SemanticMutation,
} from './encode-mutations.js';

// `encodeMutations` is the single pure wire-format producer for the
// CanvasRenderer → Hayate WASM boundary (issue #237). These tests drive the
// encode directly — no `RawHayate` stub, no WASM crossing.

describe('encodeMutations', () => {
  it('encodes createElement as [OP.CREATE, id, kind] with empty style/text buffers', () => {
    const { ops, styles, texts } = encodeMutations([
      { kind: 'createElement', id: 7 as never, elementKind: 'view' },
    ]);
    expect(Array.from(ops)).toEqual([OP.CREATE, 7, ELEMENT_KIND['view']]);
    expect(Array.from(styles)).toEqual([]);
    expect(texts).toEqual([]);
  });
});

describe('encodeMutations – setStyleVariant per-property split (ADR-0081)', () => {
  it('splits a multi-property variant into one op per property, sharing the condition', () => {
    const { ops, styles } = encodeMutations([
      {
        kind: 'setStyleVariant',
        id: 3 as never,
        condition: { maxWidth: 600 },
        style: { width: '100px', height: '200px' },
      },
    ]);
    // Two SET_STYLE_VARIANT ops, each carrying exactly one property's style.
    expect(Array.from(ops)).toEqual([
      OP.SET_STYLE_VARIANT, 3, -1, 600, -1, -1, /* offset */ 0, /* len */ 3,
      OP.SET_STYLE_VARIANT, 3, -1, 600, -1, -1, /* offset */ 3, /* len */ 3,
    ]);
    // Styles are packed contiguously: width slice then height slice.
    expect(Array.from(styles)).toEqual([
      TAG.WIDTH, 100, 0,
      TAG.HEIGHT, 200, 0,
    ]);
  });

  it('emits no op when the variant patch has no defined property', () => {
    const { ops, styles } = encodeMutations([
      {
        kind: 'setStyleVariant',
        id: 3 as never,
        condition: { maxWidth: 600 },
        style: { width: undefined },
      },
    ]);
    expect(Array.from(ops)).toEqual([]);
    expect(Array.from(styles)).toEqual([]);
  });
});

describe('viewportAxis (ADR-0081)', () => {
  it('passes a present axis value through unchanged', () => {
    expect(viewportAxis(600)).toBe(600);
    expect(viewportAxis(0)).toBe(0);
  });

  it('encodes an unset axis as -1', () => {
    expect(viewportAxis(undefined)).toBe(-1);
  });
});

describe('splitStyleVariant (ADR-0081)', () => {
  it('returns one single-property patch per defined key, in declaration order', () => {
    expect(splitStyleVariant({ width: '100px', height: '200px' })).toEqual([
      { width: '100px' },
      { height: '200px' },
    ]);
  });

  it('drops undefined entries', () => {
    expect(
      splitStyleVariant({ width: undefined, height: '200px' }),
    ).toEqual([{ height: '200px' }]);
  });

  it('returns an empty list for an empty patch', () => {
    expect(splitStyleVariant({})).toEqual([]);
  });
});

describe('encodeMutations – setStyle', () => {
  it('packs the encoded style and references it by offset/len', () => {
    const { ops, styles } = encodeMutations([
      { kind: 'setStyle', id: 5 as never, style: { width: '100px' } },
    ]);
    expect(Array.from(ops)).toEqual([OP.SET_STYLE, 5, 0, 3]);
    expect(Array.from(styles)).toEqual([TAG.WIDTH, 100, 0]);
  });

  it('emits an UNSET_STYLE op for a null inherited reset', () => {
    const { ops, styles } = encodeMutations([
      { kind: 'setStyle', id: 5 as never, style: { color: null } },
    ]);
    expect(Array.from(ops)).toEqual([OP.UNSET_STYLE, 5, UNSET_KIND.color]);
    expect(Array.from(styles)).toEqual([]);
  });
});

describe('encodeMutations – text buffer', () => {
  it('assigns ascending text indices across text-bearing ops', () => {
    const { ops, texts } = encodeMutations([
      { kind: 'setText', id: 1 as never, text: 'a' },
      { kind: 'setTextContent', id: 2 as never, text: 'b' },
      { kind: 'setSrc', id: 3 as never, url: 'c' },
    ]);
    expect(texts).toEqual(['a', 'b', 'c']);
    expect(Array.from(ops)).toEqual([
      OP.SET_TEXT, 1, 0,
      OP.SET_TEXT_CONTENT, 2, 1,
      OP.SET_SRC, 3, 2,
    ]);
  });
});
