import { describe, it, expect, vi } from 'vitest';
import { CanvasRenderer } from './canvas-renderer.js';
import type { RawHayate } from './hayate.js';

class StubHayate implements RawHayate {
  mutations: Array<{ ops: number[]; styles: number[]; texts: string[] }> = [];
  renders: number[] = [];
  events: unknown[][] = [];
  listenerSeq = 1;
  registeredListeners: Array<{ elementId: number; eventKind: number; listenerId: number }> =
    [];

  element_create(): void {}
  set_root(): void {}
  element_set_text(): void {}
  element_get_text(): string {
    return '';
  }
  element_append_child(): void {}
  element_insert_before(): void {}
  element_remove(): void {}
  element_subtree_ids(): Float64Array {
    return new Float64Array();
  }
  element_set_style(): void {}
  element_set_pseudo_style(): void {}
  apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[]): void {
    this.mutations.push({
      ops: Array.from(ops),
      styles: Array.from(styles),
      texts: Array.from(texts),
    });
  }
  on_resize(): void {}
  on_pointer_move(): void {}
  on_pointer_down(): void {}
  on_pointer_up(): void {}
  on_wheel(): void {}
  on_key_down(): void {}
  on_text_input(): void {}
  on_composition_start(): void {}
  on_composition_update(): void {}
  on_composition_end(): void {}
  focused_element_id(): number {
    return 0;
  }
  ime_character_bounds(): number[] {
    return [0, 0, 0, 0];
  }
  render(timestampMs: number): void {
    this.renders.push(timestampMs);
  }
  poll_events(): unknown[] {
    const current = this.events;
    this.events = [];
    return current;
  }
  register_listener(elementId: number, eventKind: number): number {
    const listenerId = this.listenerSeq++;
    this.registeredListeners.push({ elementId, eventKind, listenerId });
    return listenerId;
  }
  set_background_color(): void {}
}

function manualScheduler() {
  let pending: FrameRequestCallback | null = null;
  return {
    requestFrame: (cb: FrameRequestCallback) => {
      pending = cb;
      return 1;
    },
    cancelFrame: () => {
      pending = null;
    },
    tick: (timestamp = 16) => {
      const cb = pending;
      pending = null;
      cb?.(timestamp);
    },
  };
}

// Delivery poll only — apply_mutations wire integration (C3) lives in wasm-integration.test.ts.
describe('CanvasRenderer delivery poll (ADR-0053)', () => {
  it('registers Hayate listeners and dispatches poll deliveries', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);

    const button = renderer.createElement('button');
    const label = renderer.createElement('text');
    renderer.appendChild(button, label);

    const received: unknown[] = [];
    renderer.addEventListener(button, 'click', (event) => received.push(event));

    expect(hayate.registeredListeners).toEqual([
      { elementId: 1, eventKind: 0, listenerId: 1 },
    ]);

    hayate.events = [[1, 0, 2, 10, 20]];
    sched.tick();

    expect(received).toEqual([{ kind: 'click', target: 2 }]);
  });

  it('removeChild requires adapter unsubscribe before stale deliveries stop', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);

    const parent = renderer.createElement('view');
    const child = renderer.createElement('view');
    const grandchild = renderer.createElement('text');
    renderer.appendChild(parent, child);
    renderer.appendChild(child, grandchild);

    const handler = vi.fn();
    const unsub = renderer.addEventListener(grandchild, 'click', handler);
    renderer.removeChild(parent, child);
    unsub();

    hayate.events = [[1, 0, 3, 0, 0]];
    sched.tick();
    expect(handler).not.toHaveBeenCalled();
  });

  it('unsubscribe stops delivery dispatch', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const node = renderer.createElement('button');

    const handler = vi.fn();
    const unsub = renderer.addEventListener(node, 'click', handler);
    unsub();

    hayate.events = [[1, 0, 1, 0, 0]];
    sched.tick();
    expect(handler).not.toHaveBeenCalled();
  });
});
