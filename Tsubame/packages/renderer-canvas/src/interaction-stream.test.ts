import { describe, it, expect, vi } from 'vitest';
import { createInteractionStream } from './interaction-stream.js';
import type { InteractionStreamOptions } from './interaction-stream.js';

// ── Test helpers ──────────────────────────────────────────────────────────────

type ElementId = number & { __brand: 'ElementId' };
const id = (n: number): ElementId => n as ElementId;

/** Build a flat parent chain: child → parent → grandparent. */
function makeOptions(
  parentMap: Record<number, number> = {},
): InteractionStreamOptions & { handlers: Map<number, Map<string, ReturnType<typeof vi.fn>>> } {
  const handlers = new Map<number, Map<string, ReturnType<typeof vi.fn>>>();

  return {
    handlers,
    getParent: (nodeId) => {
      const p = parentMap[nodeId];
      return p !== undefined ? id(p) : undefined;
    },
    getHandlers: (nodeId, kind) => {
      const byKind = handlers.get(nodeId);
      if (byKind === undefined) return undefined;
      const set = byKind.get(kind);
      return set !== undefined ? [set] as unknown as Iterable<typeof set> : undefined;
    },
  };
}

function addHandler(
  opts: ReturnType<typeof makeOptions>,
  nodeId: number,
  kind: string,
) {
  let byKind = opts.handlers.get(nodeId);
  if (byKind === undefined) {
    byKind = new Map();
    opts.handlers.set(nodeId, byKind);
  }
  const fn = vi.fn();
  byKind.set(kind, fn);
  return fn;
}

// ── Bubbling: click / input / keydown ─────────────────────────────────────────

describe('dispatchOne – bubbling events', () => {
  it('click bubbles up through parent chain', () => {
    // 3 → 2 → 1 (root)
    const opts = makeOptions({ 3: 2, 2: 1 });
    const h1 = addHandler(opts, 1, 'click');
    const h2 = addHandler(opts, 2, 'click');
    const h3 = addHandler(opts, 3, 'click');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('click', id(3));

    expect(h3).toHaveBeenCalledTimes(1);
    expect(h2).toHaveBeenCalledTimes(1);
    expect(h1).toHaveBeenCalledTimes(1);
  });

  it('input bubbles up through parent chain', () => {
    const opts = makeOptions({ 2: 1 });
    const h1 = addHandler(opts, 1, 'input');
    const h2 = addHandler(opts, 2, 'input');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('input', id(2), { value: 'hello' });

    expect(h2).toHaveBeenCalledWith(expect.objectContaining({ kind: 'input', value: 'hello' }));
    expect(h1).toHaveBeenCalledWith(expect.objectContaining({ kind: 'input', value: 'hello' }));
  });

  it('keydown bubbles up through parent chain', () => {
    const opts = makeOptions({ 2: 1 });
    const h1 = addHandler(opts, 1, 'keydown');
    const h2 = addHandler(opts, 2, 'keydown');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('keydown', id(2), { key: 'Enter' });

    expect(h2).toHaveBeenCalledWith(expect.objectContaining({ kind: 'keydown', key: 'Enter' }));
    expect(h1).toHaveBeenCalledWith(expect.objectContaining({ kind: 'keydown', key: 'Enter' }));
  });

  it('change bubbles up through parent chain', () => {
    const opts = makeOptions({ 2: 1 });
    const h1 = addHandler(opts, 1, 'change');
    const h2 = addHandler(opts, 2, 'change');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('change', id(2));

    expect(h2).toHaveBeenCalledTimes(1);
    expect(h1).toHaveBeenCalledTimes(1);
  });
});

// ── Non-bubbling: focus / blur / hover-enter / hover-leave ───────────────────

describe('dispatchOne – non-bubbling events', () => {
  it('focus does NOT bubble', () => {
    const opts = makeOptions({ 2: 1 });
    const h1 = addHandler(opts, 1, 'focus');
    const h2 = addHandler(opts, 2, 'focus');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('focus', id(2));

    expect(h2).toHaveBeenCalledTimes(1);
    expect(h1).not.toHaveBeenCalled();
  });

  it('blur does NOT bubble', () => {
    const opts = makeOptions({ 2: 1 });
    const h1 = addHandler(opts, 1, 'blur');
    const h2 = addHandler(opts, 2, 'blur');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('blur', id(2));

    expect(h2).toHaveBeenCalledTimes(1);
    expect(h1).not.toHaveBeenCalled();
  });

  it('hover-enter does NOT bubble', () => {
    const opts = makeOptions({ 2: 1 });
    const h1 = addHandler(opts, 1, 'hover-enter');
    const h2 = addHandler(opts, 2, 'hover-enter');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('hover-enter', id(2));

    expect(h2).toHaveBeenCalledTimes(1);
    expect(h1).not.toHaveBeenCalled();
  });

  it('hover-leave does NOT bubble', () => {
    const opts = makeOptions({ 2: 1 });
    const h1 = addHandler(opts, 1, 'hover-leave');
    const h2 = addHandler(opts, 2, 'hover-leave');
    const stream = createInteractionStream(opts);

    stream.dispatchOne('hover-leave', id(2));

    expect(h2).toHaveBeenCalledTimes(1);
    expect(h1).not.toHaveBeenCalled();
  });
});

// ── dispatchParsedEvent – EventKind mappings ──────────────────────────────────

describe('dispatchParsedEvent – event kind mapping', () => {
  it('text_input maps to "input" EventKind with value', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 5, 'input');
    const stream = createInteractionStream(opts);

    stream.dispatchParsedEvent({ kind: 'text_input', value: 3, targetId: 5, text: 'abc' });

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'input', value: 'abc' }));
  });

  it('key_down maps to "keydown" EventKind with key', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 5, 'keydown');
    const stream = createInteractionStream(opts);

    stream.dispatchParsedEvent({ kind: 'key_down', value: 12, targetId: 5, key: 'ArrowUp', modifiers: 0 });

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'keydown', key: 'ArrowUp' }));
  });

  it('click maps to "click"', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 7, 'click');
    const stream = createInteractionStream(opts);

    stream.dispatchParsedEvent({ kind: 'click', value: 0, targetId: 7, x: 10, y: 20 });

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'click', target: 7 }));
  });

  it('hover_enter maps to "hover-enter"', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 3, 'hover-enter');
    const stream = createInteractionStream(opts);

    stream.dispatchParsedEvent({ kind: 'hover_enter', value: 10, targetId: 3 });

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'hover-enter', target: 3 }));
  });

  it('hover_leave maps to "hover-leave"', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 3, 'hover-leave');
    const stream = createInteractionStream(opts);

    stream.dispatchParsedEvent({ kind: 'hover_leave', value: 11, targetId: 3 });

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'hover-leave', target: 3 }));
  });

  it('focus maps to "focus"', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 4, 'focus');
    const stream = createInteractionStream(opts);

    stream.dispatchParsedEvent({ kind: 'focus', value: 1, targetId: 4 });

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'focus', target: 4 }));
  });

  it('blur maps to "blur"', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 4, 'blur');
    const stream = createInteractionStream(opts);

    stream.dispatchParsedEvent({ kind: 'blur', value: 2, targetId: 4 });

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'blur', target: 4 }));
  });
});

// ── dispatchParsedEvent – ignored event kinds ─────────────────────────────────

describe('dispatchParsedEvent – ignored events call no handlers', () => {
  function makeAnyHandlerOpts() {
    const opts = makeOptions();
    // Place catch-all handlers at node 1 for every EventKind
    const allKinds = ['click', 'input', 'focus', 'blur', 'hover-enter', 'hover-leave', 'keydown', 'change'];
    const fns: ReturnType<typeof vi.fn>[] = [];
    for (const k of allKinds) {
      fns.push(addHandler(opts, 1, k));
    }
    return { opts, fns };
  }

  it('composition_start is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'composition_start', value: 4, targetId: 1, text: 'a' });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('composition_update is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'composition_update', value: 5, targetId: 1, text: 'b' });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('composition_end is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'composition_end', value: 6, targetId: 1, text: 'c' });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('scroll is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'scroll', value: 7, targetId: 1, deltaX: 0, deltaY: 10 });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('resize is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'resize', value: 8, width: 800, height: 600 });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('active_start is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'active_start', value: 13, targetId: 1 });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('active_end is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'active_end', value: 9, targetId: 1 });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('pointer_move is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'pointer_move', value: 14, x: 5, y: 10 });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });

  it('fetch_font is ignored', () => {
    const { opts, fns } = makeAnyHandlerOpts();
    const stream = createInteractionStream(opts);
    stream.dispatchParsedEvent({ kind: 'fetch_font', value: 15, family: 'Inter' });
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
  });
});

// ── dispatchRawEvents – parseEvent integration ────────────────────────────────

describe('dispatchRawEvents', () => {
  it('dispatches click from raw array [0, targetId, x, y]', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 9, 'click');
    const stream = createInteractionStream(opts);

    stream.dispatchRawEvents([[0, 9, 5, 10]]);

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'click', target: 9 }));
  });

  it('dispatches text_input → "input" from raw array [3, targetId, text]', () => {
    const opts = makeOptions();
    const h = addHandler(opts, 2, 'input');
    const stream = createInteractionStream(opts);

    stream.dispatchRawEvents([[3, 2, 'typed text']]);

    expect(h).toHaveBeenCalledWith(expect.objectContaining({ kind: 'input', value: 'typed text' }));
  });

  it('dispatches multiple events in order', () => {
    const opts = makeOptions({ 2: 1 });
    const clickH = addHandler(opts, 2, 'click');
    const inputH = addHandler(opts, 2, 'input');
    const stream = createInteractionStream(opts);

    stream.dispatchRawEvents([
      [0, 2, 0, 0],        // click at node 2
      [3, 2, 'hi'],        // text_input at node 2
    ]);

    expect(clickH).toHaveBeenCalledTimes(1);
    expect(inputH).toHaveBeenCalledTimes(1);
  });

  it('unknown event kind throws from parseEvent', () => {
    const opts = makeOptions();
    const stream = createInteractionStream(opts);
    expect(() => stream.dispatchRawEvents([[999, 1]])).toThrow();
  });
});
