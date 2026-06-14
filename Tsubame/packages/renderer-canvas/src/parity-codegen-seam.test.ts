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
  it('renderer-canvas parse helpers are the generated codec functions, not re-implementations', () => {
    // Single source: the canvas package must expose the *same function objects*
    // the codegen produced, so a colour-parse fix lands in exactly one place.
    expect(parseColor).toBe(codecParseColor);
    expect(parseDimension).toBe(codecParseDimension);
    expect(finiteNumber).toBe(codecFiniteNumber);
    expect(finiteInteger).toBe(codecFiniteInteger);
    expect(canvasPkg.parseColor).toBe(codecParseColor);
  });

  it('coerceElementProperty is the single source for setProperty value semantics', () => {
    // `value` → editable text-content: null/undefined erase, everything else stringifies.
    expect(coerceElementProperty('value', 'hi')).toEqual({ kind: 'text-content', text: 'hi' });
    expect(coerceElementProperty('value', null)).toEqual({ kind: 'text-content', text: '' });
    expect(coerceElementProperty('value', undefined)).toEqual({ kind: 'text-content', text: '' });
    expect(coerceElementProperty('value', 42)).toEqual({ kind: 'text-content', text: '42' });
    expect(coerceElementProperty('value', true)).toEqual({ kind: 'text-content', text: 'true' });

    // `placeholder` / `src` → text, but only honour real strings (non-strings erase).
    expect(coerceElementProperty('placeholder', 'p')).toEqual({ kind: 'placeholder', text: 'p' });
    expect(coerceElementProperty('placeholder', 42)).toEqual({ kind: 'placeholder', text: '' });
    expect(coerceElementProperty('src', 'http://x/a.png')).toEqual({ kind: 'src', text: 'http://x/a.png' });
    expect(coerceElementProperty('src', null)).toEqual({ kind: 'src', text: '' });

    // `disabled` → boolean flag via Boolean() (note: the string 'false' is truthy).
    expect(coerceElementProperty('disabled', true)).toEqual({ kind: 'disabled', disabled: true });
    expect(coerceElementProperty('disabled', 0)).toEqual({ kind: 'disabled', disabled: false });
    expect(coerceElementProperty('disabled', '')).toEqual({ kind: 'disabled', disabled: false });
    expect(coerceElementProperty('disabled', 'false')).toEqual({ kind: 'disabled', disabled: true });
  });
});
