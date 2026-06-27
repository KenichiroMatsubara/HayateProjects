import { describe, it, expect } from 'vitest';
import {
  parseColor as codecParseColor,
  parseDimension as codecParseDimension,
  finiteNumber as codecFiniteNumber,
  finiteInteger as codecFiniteInteger,
} from '@tsubame/protocol-generated/codec';
import { coerceElementProperty } from '@tsubame/renderer-protocol';
import * as canvasPkg from './index.js';
import {
  parseColor,
  parseDimension,
  finiteNumber,
  finiteInteger,
} from './hayate.js';

describe('parity codegen seam (issue #235)', () => {
  it('renderer-hayate parse helpers are the generated codec functions, not re-implementations', () => {
    // 単一の出所: canvas パッケージは codegen が生成したのと同一の関数オブジェクトを
    // 公開しなければならない。色パースの修正が一箇所で済むようにするため。
    expect(parseColor).toBe(codecParseColor);
    expect(parseDimension).toBe(codecParseDimension);
    expect(finiteNumber).toBe(codecFiniteNumber);
    expect(finiteInteger).toBe(codecFiniteInteger);
    expect(canvasPkg.parseColor).toBe(codecParseColor);
  });

  it('coerceElementProperty is the single source for setProperty value semantics', () => {
    // `value` → 編集可能な text-content: null/undefined は消去、それ以外は文字列化。
    expect(coerceElementProperty('value', 'hi')).toEqual({ kind: 'text-content', text: 'hi' });
    expect(coerceElementProperty('value', null)).toEqual({ kind: 'text-content', text: '' });
    expect(coerceElementProperty('value', undefined)).toEqual({ kind: 'text-content', text: '' });
    expect(coerceElementProperty('value', 42)).toEqual({ kind: 'text-content', text: '42' });
    expect(coerceElementProperty('value', true)).toEqual({ kind: 'text-content', text: 'true' });

    // `placeholder` / `src` → text。ただし文字列のみ採用（非文字列は消去）。
    expect(coerceElementProperty('placeholder', 'p')).toEqual({ kind: 'placeholder', text: 'p' });
    expect(coerceElementProperty('placeholder', 42)).toEqual({ kind: 'placeholder', text: '' });
    expect(coerceElementProperty('src', 'http://x/a.png')).toEqual({ kind: 'src', text: 'http://x/a.png' });
    expect(coerceElementProperty('src', null)).toEqual({ kind: 'src', text: '' });

    // `disabled` → Boolean() で真偽フラグ化（文字列 'false' は truthy になる点に注意）。
    expect(coerceElementProperty('disabled', true)).toEqual({ kind: 'disabled', disabled: true });
    expect(coerceElementProperty('disabled', 0)).toEqual({ kind: 'disabled', disabled: false });
    expect(coerceElementProperty('disabled', '')).toEqual({ kind: 'disabled', disabled: false });
    expect(coerceElementProperty('disabled', 'false')).toEqual({ kind: 'disabled', disabled: true });
  });
});
