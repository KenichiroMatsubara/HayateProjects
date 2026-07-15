import { describe, expect, it, vi } from 'vitest';
import { dispatchEditIntentWithKeyFallback } from './edit-intent.js';
import { StubHayate } from './test-helpers/stub-hayate.js';

describe('public EditIntent capability (#828)', () => {
  it.each([
    [0, 'consumed', 0],
    [1, 'unhandled', 1],
    [2, 'deferred', 0],
  ] as const)('projects outcome %i and falls back only for Unhandled', (wire, outcome, fallbacks) => {
    const raw = new StubHayate();
    raw.dispatch_edit_intent = vi.fn(() => wire);
    raw.on_key_down = vi.fn();

    expect(
      dispatchEditIntentWithKeyFallback(raw, 1, { kind: 'selectAll' }, 'a', 2),
    ).toBe(outcome);
    expect(raw.on_key_down).toHaveBeenCalledTimes(fallbacks);
    expect(raw.mutations).toEqual([]);
  });
});
