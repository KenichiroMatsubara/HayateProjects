import { describe, it, expect } from 'vitest';
import {
  coerceElementProperty,
  dispatchElementPropertyOp,
  type ElementPropertyEffects,
} from '@tsubame/renderer-protocol';

describe('dispatchElementPropertyOp (shared prop-op dispatch, ADR-0008)', () => {
  it('routes a coerced op to the effect handler for its kind, passing the op', () => {
    const op = coerceElementProperty('value', 'hi');
    const seen: string[] = [];
    const effects: ElementPropertyEffects<string> = {
      'text-content': (o) => {
        seen.push(o.kind);
        return o.text;
      },
      placeholder: () => 'no',
      src: () => 'no',
      disabled: () => 'no',
      'user-select': () => 'no',
      multiline: () => 'no',
    };
    expect(dispatchElementPropertyOp(op, effects)).toBe('hi');
    expect(seen).toEqual(['text-content']);
  });

  it('selects the boolean-payload handler for a boolean op-kind', () => {
    const op = coerceElementProperty('disabled', 'false');
    const result = dispatchElementPropertyOp<boolean>(op, {
      'text-content': () => false,
      placeholder: () => false,
      src: () => false,
      disabled: (o) => o.disabled,
      'user-select': () => false,
      multiline: () => false,
    });
    // 'false' is truthy → Boolean('false') === true, reflected through the seam.
    expect(result).toBe(true);
  });
});
