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
  private readonly parentOf = new Map<number, number>();
  private readonly childrenOf = new Map<number, Set<number>>();

  element_create(): void {}
  set_root(): void {}
  element_set_text(): void {}

  element_append_child(parent: number, child: number): void {
    this.linkParent(parent, child);
  }

  element_insert_before(parent: number, child: number, _before: number): void {
    this.linkParent(parent, child);
  }

  element_remove(root: number): void {
    const stack = [root];
    while (stack.length > 0) {
      const node = stack.pop()!;
      const children = this.childrenOf.get(node);
      if (children !== undefined) {
        for (const child of children) {
          this.parentOf.delete(child);
          stack.push(child);
        }
        this.childrenOf.delete(node);
      }
      const parent = this.parentOf.get(node);
      if (parent !== undefined) {
        this.childrenOf.get(parent)?.delete(node);
        this.parentOf.delete(node);
      }
    }
  }

  element_subtree_ids(root: number): number[] {
    const ids: number[] = [];
    const stack = [root];
    while (stack.length > 0) {
      const node = stack.pop()!;
      ids.push(node);
      const children = this.childrenOf.get(node);
      if (children !== undefined) {
        for (const child of children) {
          stack.push(child);
        }
      }
    }
    return ids;
  }

  element_set_style(): void {}

  private linkParent(parent: number, child: number): void {
    const prevParent = this.parentOf.get(child);
    if (prevParent !== undefined) {
      this.childrenOf.get(prevParent)?.delete(child);
    }
    this.parentOf.set(child, parent);
    let children = this.childrenOf.get(parent);
    if (children === undefined) {
      children = new Set();
      this.childrenOf.set(parent, children);
    }
    children.add(child);
  }
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

  it('ignores deliveries for unknown listener ids after subtree removal', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);

    const parent = renderer.createElement('view');
    const child = renderer.createElement('view');
    const grandchild = renderer.createElement('text');
    renderer.appendChild(parent, child);
    renderer.appendChild(child, grandchild);

    const handler = vi.fn();
    renderer.addEventListener(grandchild, 'click', handler);
    renderer.removeChild(parent, child);

    hayate.events = [[1, 0, 3, 0, 0]];
    expect(() => sched.tick()).not.toThrow();
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
