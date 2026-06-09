import { afterEach, describe, expect, it } from 'vitest';
import { OP } from '@tsubame/protocol-generated/protocol';
import { CanvasRenderer } from './canvas-renderer.js';
import { createNullHayate, type WasmHayateFixture } from './test-helpers/wasm-hayate.js';

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

describe('codec integration (C3)', () => {
  let fixture: WasmHayateFixture | null = null;

  afterEach(() => {
    fixture?.dispose();
    fixture = null;
  });

  it('CanvasRenderer flush applies generated wire through real apply_mutations', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(fixture.raw, sched);

    const root = renderer.createElement('view');
    const text = renderer.createElement('text');
    renderer.setRoot(root);
    renderer.appendChild(root, text);
    renderer.setStyle(root, { width: '50%', backgroundColor: '#ff0000' });
    renderer.setText(text, 'Hello');

    sched.tick(33);

    const hayate = fixture.raw as {
      element_get_text(id: number): string;
      element_subtree_ids(id: number): Float64Array;
    };

    expect(hayate.element_get_text(2)).toBe('Hello');
    expect(Array.from(hayate.element_subtree_ids(1))).toEqual([1, 2]);
  });

  it('preserves ordered setText mutations without coalescing', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(fixture.raw, sched);

    const text = renderer.createElement('text');
    renderer.setText(text, 'A');
    renderer.setText(text, 'B');

    sched.tick(16);

    const hayate = fixture.raw as { element_get_text(id: number): string };
    expect(hayate.element_get_text(1)).toBe('B');
  });

  it('removeChild drops subtree from Hayate element tree', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(fixture.raw, sched);

    const parent = renderer.createElement('view');
    const child = renderer.createElement('view');
    const grandchild = renderer.createElement('text');
    renderer.appendChild(parent, child);
    renderer.appendChild(child, grandchild);

    sched.tick(16);
    renderer.removeChild(parent, child);
    sched.tick(32);

    const hayate = fixture.raw as {
      element_subtree_ids(id: number): Float64Array;
      element_get_text(id: number): string;
    };

    expect(Array.from(hayate.element_subtree_ids(1))).toEqual([1]);
    expect(() => hayate.element_get_text(3)).not.toThrow();
  });

  it('applies setProperty value through apply_mutations on text-input', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(fixture.raw, sched);

    const input = renderer.createElement('text-input');
    renderer.setRoot(input);
    renderer.setProperty(input, 'value', 'committed');

    sched.tick(16);

    const hayate = fixture.raw as {
      element_get_text_content(id: number): string;
    };
    expect(hayate.element_get_text_content(1)).toBe('committed');
  });

  it('matches binding.test ops wire for unified batch', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(fixture.raw, sched);

    const root = renderer.createElement('view');
    const text = renderer.createElement('text');
    renderer.setRoot(root);
    renderer.appendChild(root, text);
    renderer.setStyle(root, { width: '50%', backgroundColor: '#ff0000' });
    renderer.setText(text, 'Hello');

    const recorded: { ops: number[]; styles: number[]; texts: string[] }[] = [];
    const raw = fixture.raw;
    const original = raw.apply_mutations.bind(raw);
    raw.apply_mutations = (ops, styles, texts) => {
      recorded.push({
        ops: Array.from(ops),
        styles: Array.from(styles),
        texts: [...texts],
      });
      original(ops, styles, texts);
    };

    sched.tick(33);

    expect(recorded).toHaveLength(1);
    expect(recorded[0]!.ops).toEqual([
      OP.CREATE,
      1,
      0,
      OP.CREATE,
      2,
      1,
      OP.SET_ROOT,
      1,
      OP.APPEND_CHILD,
      1,
      2,
      OP.SET_STYLE,
      1,
      0,
      8,
      OP.SET_TEXT,
      2,
      0,
    ]);
    expect(recorded[0]!.styles).toEqual([5, 50, 1, 0, 1, 0, 0, 1]);
    expect(recorded[0]!.texts).toEqual(['Hello']);
  });
});
