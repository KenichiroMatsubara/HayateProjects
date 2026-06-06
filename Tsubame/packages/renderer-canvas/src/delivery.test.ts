import { describe, it, expect } from 'vitest';
import { parseDelivery, toInteractionEvent } from './delivery.js';

describe('parseDelivery', () => {
  it('decodes listener id prefix before event fields', () => {
    const { listenerId, event } = parseDelivery([42, 0, 3, 10, 20]);
    expect(listenerId).toBe(42);
    expect(event).toEqual({
      kind: 'click',
      value: 0,
      targetId: 3,
      x: 10,
      y: 20,
    });
  });

  it('decodes text_input delivery', () => {
    const { listenerId, event } = parseDelivery([7, 3, 5, 'hello']);
    expect(listenerId).toBe(7);
    expect(event).toEqual({
      kind: 'text_input',
      value: 3,
      targetId: 5,
      text: 'hello',
    });
  });
});

describe('toInteractionEvent', () => {
  it('maps click to InteractionEvent', () => {
    expect(
      toInteractionEvent({
        kind: 'click',
        value: 0,
        targetId: 2,
        x: 1,
        y: 2,
      }),
    ).toEqual({ kind: 'click', target: 2 });
  });

  it('maps text_input to input with value', () => {
    expect(
      toInteractionEvent({
        kind: 'text_input',
        value: 3,
        targetId: 4,
        text: 'abc',
      }),
    ).toEqual({ kind: 'input', target: 4, value: 'abc' });
  });

  it('maps key_down to keydown with key', () => {
    expect(
      toInteractionEvent({
        kind: 'key_down',
        value: 12,
        targetId: 1,
        key: 'Enter',
        modifiers: 0,
      }),
    ).toEqual({ kind: 'keydown', target: 1, key: 'Enter' });
  });

  it('maps hover events', () => {
    expect(
      toInteractionEvent({ kind: 'hover_enter', value: 10, targetId: 5 }),
    ).toEqual({ kind: 'hover-enter', target: 5 });
    expect(
      toInteractionEvent({ kind: 'hover_leave', value: 11, targetId: 5 }),
    ).toEqual({ kind: 'hover-leave', target: 5 });
  });

  it('returns null for hayate-internal kinds', () => {
    expect(
      toInteractionEvent({ kind: 'fetch_font', value: 15, family: 'Inter' }),
    ).toBeNull();
    expect(
      toInteractionEvent({
        kind: 'scroll',
        value: 7,
        targetId: 1,
        deltaX: 0,
        deltaY: 10,
      }),
    ).toBeNull();
    expect(
      toInteractionEvent({ kind: 'resize', value: 8, width: 100, height: 200 }),
    ).toBeNull();
  });
});
