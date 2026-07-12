import { afterEach, describe, expect, it } from 'vitest';
import { OP } from '@torimi/tsubame-protocol-generated/protocol';
import { HayateRenderer } from './hayate-renderer.js';
import type { RawHayate } from './hayate.js';
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

  it('HayateRenderer flush applies generated wire through real apply_mutations', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

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
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

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
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

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
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

    const input = renderer.createElement('text-input');
    renderer.setRoot(input);
    renderer.setProperty(input, 'value', 'committed');

    sched.tick(16);

    // `element_get_text_content` は RawHayate ポートから外れた（IME 配線がアダプタへ移行、
    // #474）が、WASM レンダラには残る読み取りクエリ。ここではローカル型で受けて確認する。
    const hayate = fixture.raw as unknown as {
      element_get_text_content(id: number): string;
    };
    expect(hayate.element_get_text_content(1)).toBe('committed');
  });

  it('applies batched pseudo-style through apply_mutations and resolves :hover', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

    const root = renderer.createElement('view');
    const button = renderer.createElement('button');
    renderer.setRoot(root);
    renderer.appendChild(root, button);
    renderer.setStyle(button, {
      width: '100px',
      height: '40px',
      backgroundColor: '#ffffff',
    });
    renderer.setPseudoStyle(button, ':hover', { backgroundColor: '#0000ff' });
    renderer.setPseudoStyle(button, ':active', { backgroundColor: '#ff0000' });
    renderer.setPseudoStyle(button, ':focus', { backgroundColor: '#00ff00' });

    const recorded: { ops: number[]; styles: number[]; texts: string[] }[] = [];
    const raw = fixture.raw;
    const original = raw.apply_mutations.bind(raw);
    raw.apply_mutations = (ops, styles, texts, draws) => {
      recorded.push({
        ops: Array.from(ops),
        styles: Array.from(styles),
        texts: [...texts],
      });
      original(ops, styles, texts, draws);
    };

    sched.tick(16);

    expect(recorded).toHaveLength(1);
    const batch = recorded[0]!;
    const pseudoOps = batch.ops.filter((op) => op === OP.SET_PSEUDO_STYLE);
    expect(pseudoOps).toHaveLength(3);
    expect(batch.ops).not.toContain(undefined);

    const hayate = raw as {
      on_pointer_move(x: number, y: number): void;
      on_pointer_down(x: number, y: number): void;
      render(timestampMs: number): void;
      element_get_bounds(id: number): Float32Array | number[];
    };

    hayate.on_pointer_move(50, 20);
    hayate.render(32);
    const hoveredBounds = Array.from(hayate.element_get_bounds(2));
    expect(hoveredBounds[2]).toBeGreaterThan(0);
    expect(hoveredBounds[3]).toBeGreaterThan(0);

    hayate.on_pointer_down(50, 20);
    hayate.render(48);
  });

  it('element_effective_visual resolves :hover pseudo style (ADR-0067)', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

    const root = renderer.createElement('view');
    renderer.setRoot(root);
    renderer.setStyle(root, { width: '100%', height: '100%', backgroundColor: '#ffffff' });
    renderer.setPseudoStyle(root, ':hover', { backgroundColor: '#0000ff' });

    sched.tick(16);

    const hayate = fixture.raw as RawHayate;

    const base = hayate.element_effective_visual(1);
    expect(base?.backgroundColor).toEqual({ r: 1, g: 1, b: 1, a: 1 });

    hayate.on_pointer_move(50, 20);
    hayate.render(32);

    const hovered = hayate.element_effective_visual(1);
    expect(hovered?.backgroundColor).toEqual({ r: 0, g: 0, b: 1, a: 1 });
  });

  it('matches binding.test ops wire for unified batch', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

    const root = renderer.createElement('view');
    const text = renderer.createElement('text');
    renderer.setRoot(root);
    renderer.appendChild(root, text);
    renderer.setStyle(root, { width: '50%', backgroundColor: '#ff0000' });
    renderer.setText(text, 'Hello');

    const recorded: { ops: number[]; styles: number[]; texts: string[] }[] = [];
    const raw = fixture.raw;
    const original = raw.apply_mutations.bind(raw);
    raw.apply_mutations = (ops, styles, texts, draws) => {
      recorded.push({
        ops: Array.from(ops),
        styles: Array.from(styles),
        texts: [...texts],
      });
      original(ops, styles, texts, draws);
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
