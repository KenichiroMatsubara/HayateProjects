import { describe, it, expect } from 'vitest';
import { OP, TAG, ELEMENT_KIND } from '@tsubame/protocol-generated/protocol';
import { UNSET_KIND } from '@tsubame/protocol-generated/protocol';
import {
  encodeMutations,
  splitStyleVariant,
  viewportAxis,
  type SemanticMutation,
} from './encode-mutations.js';

// `encodeMutations` は HayateRenderer → Hayate WASM 境界で唯一の純粋な
// ワイヤーフォーマット生成器。これらのテストはエンコードを直接駆動し、
// `RawHayate` スタブも WASM 越えも使わない。

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
    // SET_STYLE_VARIANT op が2つ、各々ちょうど1プロパティ分の style を運ぶ。
    expect(Array.from(ops)).toEqual([
      OP.SET_STYLE_VARIANT, 3, -1, 600, -1, -1, /* offset */ 0, /* len */ 3,
      OP.SET_STYLE_VARIANT, 3, -1, 600, -1, -1, /* offset */ 3, /* len */ 3,
    ]);
    // style は連続して詰められる: width スライス、続いて height スライス。
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

describe('encodeMutations – draws channel (#724 / ADR-0141)', () => {
  it('encodes setDraw as [OP.SET_DRAW, id, offset, len] with the list in the draws buffer', () => {
    const list = [0, 10, 10, 1, 90, 10, 2, 3, 5, 0, 1, 0, 0, 1];
    const { ops, draws } = encodeMutations([
      { kind: 'setDraw', id: 5 as never, list },
    ]);
    expect(Array.from(ops)).toEqual([OP.SET_DRAW, 5, 0, list.length]);
    expect(Array.from(draws)).toEqual(list);
  });

  it('packs consecutive setDraw lists back to back with offset references', () => {
    const a = [0, 0, 0, 3, 0];
    const b = [0, 1, 1, 3, 0];
    const { ops, draws } = encodeMutations([
      { kind: 'setDraw', id: 1 as never, list: a },
      { kind: 'setDraw', id: 2 as never, list: b },
    ]);
    expect(Array.from(ops)).toEqual([
      OP.SET_DRAW, 1, 0, a.length,
      OP.SET_DRAW, 2, a.length, b.length,
    ]);
    expect(Array.from(draws)).toEqual([...a, ...b]);
  });

  it('emits an empty draws buffer when no setDraw mutations are queued', () => {
    const { draws } = encodeMutations([
      { kind: 'createElement', id: 7 as never, elementKind: 'view' },
    ]);
    expect(Array.from(draws)).toEqual([]);
  });
});
