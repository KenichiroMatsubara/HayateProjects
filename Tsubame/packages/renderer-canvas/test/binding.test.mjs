import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  CanvasRenderer,
  OP,
  parseColor,
  encodeStylePatch,
  unsetKindsOf,
} from '../dist/index.js';

/** Records the WIT-shaped calls the real WASM would receive. */
class StubHayate {
  mutations = []; // { ops: number[], styles: number[], texts: string[] }
  renders = [];
  resizes = [];
  events = [];
  calls = [];
  listenerSeq = 1;
  registeredListeners = []; // { elementId, eventKind, listenerId }

  apply_mutations(ops, styles, texts) {
    this.calls.push('apply_mutations');
    this.mutations.push({
      ops: Array.from(ops),
      styles: Array.from(styles),
      texts: Array.from(texts),
    });
  }
  on_resize(width, height) {
    this.resizes.push({ width, height });
  }
  render(timestampMs) {
    this.renders.push(timestampMs);
  }
  poll_events() {
    const current = this.events;
    this.events = [];
    return current;
  }
  register_listener(elementId, eventKind) {
    const listenerId = this.listenerSeq++;
    this.registeredListeners.push({ elementId, eventKind, listenerId });
    this.calls.push('register_listener');
    return listenerId;
  }
}

function manualScheduler() {
  let pending = null;
  return {
    requestFrame: (cb) => {
      pending = cb;
      return 1;
    },
    cancelFrame: () => {
      pending = null;
    },
    tick: (timestamp = 16) => {
      const cb = pending;
      pending = null;
      if (cb) cb(timestamp);
    },
  };
}

test('CanvasRenderer batches tree, style, and text mutations into unified apply_mutations (ADR-0052)', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const root = renderer.createElement('view');
  const text = renderer.createElement('text');
  renderer.setRoot(root);
  renderer.appendChild(root, text);
  renderer.setStyle(root, { width: '50%', backgroundColor: '#ff0000' });
  renderer.setText(text, 'Hello');

  assert.equal(hayate.mutations.length, 0);

  sched.tick(33);

  assert.equal(hayate.mutations.length, 1);
  assert.deepEqual(hayate.mutations[0].ops, [
    OP.CREATE, 1, 0, // create view #1
    OP.CREATE, 2, 1, // create text #2
    OP.SET_ROOT, 1, // set_root #1
    OP.APPEND_CHILD, 1, 2, // append #2 under #1
    OP.SET_STYLE, 1, 0, 8, // set_style #1, offset 0, len 8
    OP.SET_TEXT, 2, 0, // set_text #2, texts[0]
  ]);
  // TAG_WIDTH=5 (50, percent=1), TAG_BACKGROUND_COLOR=0 (r,g,b,a)
  assert.deepEqual(hayate.mutations[0].styles, [5, 50, 1, 0, 1, 0, 0, 1]);
  assert.deepEqual(hayate.mutations[0].texts, ['Hello']);
  assert.deepEqual(hayate.calls, ['apply_mutations']);
  assert.deepEqual(hayate.renders, [33]);
});

test('setStyle null resets route to unified OP_UNSET_STYLE records', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const node = renderer.createElement('text');
  renderer.setStyle(node, { color: null, fontSize: null, fontFamily: null });

  sched.tick(16);

  // color=0, font-size=1, font-family=2
  assert.deepEqual(hayate.calls, ['apply_mutations']);
  assert.deepEqual(hayate.mutations[0].ops, [
    OP.CREATE, 1, 1,
    OP.UNSET_STYLE, 1, 0,
    OP.UNSET_STYLE, 1, 1,
    OP.UNSET_STYLE, 1, 2,
  ]);
  assert.deepEqual(hayate.mutations[0].styles, []);
  assert.deepEqual(hayate.mutations[0].texts, []);
});

test('CanvasRenderer preserves text before later style mutations in one unified batch', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const text = renderer.createElement('text');
  renderer.setText(text, 'Hello');
  renderer.setStyle(text, { color: '#00ff00' });

  assert.equal(hayate.mutations.length, 0);

  sched.tick(24);

  assert.equal(hayate.mutations.length, 1);
  assert.deepEqual(hayate.mutations[0].ops, [
    OP.CREATE, 1, 1,
    OP.SET_TEXT, 1, 0,
    OP.SET_STYLE, 1, 0, 5,
  ]);
  assert.deepEqual(hayate.mutations[0].styles, [27, 0, 1, 0, 1]);
  assert.deepEqual(hayate.mutations[0].texts, ['Hello']);
  assert.deepEqual(hayate.calls, ['apply_mutations']);
});

test('setStyle with SET and null unset emits ordered SET_STYLE then OP_UNSET_STYLE', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const node = renderer.createElement('text');
  renderer.setStyle(node, { color: '#ff0000', fontSize: null });

  sched.tick(16);

  assert.deepEqual(hayate.calls, ['apply_mutations']);
  assert.deepEqual(hayate.mutations[0].ops, [
    OP.CREATE, 1, 1,
    OP.SET_STYLE, 1, 0, 5,
    OP.UNSET_STYLE, 1, 1,
  ]);
  assert.deepEqual(hayate.mutations[0].styles, [27, 1, 0, 0, 1]);
  assert.deepEqual(hayate.mutations[0].texts, []);
});

test('multiple setText calls are emitted in order without coalescing', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const text = renderer.createElement('text');
  renderer.setText(text, 'A');
  renderer.setText(text, 'B');

  sched.tick(16);

  assert.deepEqual(hayate.mutations[0].ops, [
    OP.CREATE, 1, 1,
    OP.SET_TEXT, 1, 0,
    OP.SET_TEXT, 1, 1,
  ]);
  assert.deepEqual(hayate.mutations[0].texts, ['A', 'B']);
  assert.deepEqual(hayate.calls, ['apply_mutations']);
});

test('encodeStylePatch mirrors the style_packet TAG format', () => {
  const dims = [];
  encodeStylePatch({ width: '50%', backgroundColor: '#ff0000' }, dims);
  assert.deepEqual(dims, [5, 50, 1, 0, 1, 0, 0, 1]);

  const mixed = [];
  encodeStylePatch(
    {
      display: 'flex',
      flexDirection: 'column',
      flexGrow: 1,
      fontWeight: 700,
      padding: 12,
      gap: 'auto',
    },
    mixed,
  );
  // DISPLAY=11(flex=0), FLEX_DIRECTION=12(column=1), FLEX_GROW=30,
  // FONT_WEIGHT=31, PADDING=16(12, px=0), GAP=15(0, auto=2)
  assert.deepEqual(mixed, [11, 0, 12, 1, 30, 1, 31, 700, 16, 12, 0, 15, 0, 2]);

  const family = [];
  encodeStylePatch({ fontFamily: 'AB' }, family);
  // FONT_FAMILY=29, len=2, 'A'=65, 'B'=66
  assert.deepEqual(family, [29, 2, 65, 66]);
});

test('unsetKindsOf maps inherited nulls and rejects others', () => {
  assert.deepEqual(
    unsetKindsOf({ color: null, fontSize: null, fontFamily: null, fontWeight: null }),
    [0, 1, 2, 3],
  );
  assert.throws(() => unsetKindsOf({ width: null }), /cannot reset/);
});

test('parseColor keeps colour records explicit', () => {
  assert.deepEqual(parseColor('#112233'), {
    r: 0x11 / 255,
    g: 0x22 / 255,
    b: 0x33 / 255,
    a: 1,
  });
});

test('CanvasRenderer registers listeners and dispatches poll deliveries (ADR-0053)', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);
  const button = renderer.createElement('button');
  const label = renderer.createElement('text');
  renderer.appendChild(button, label);

  const received = [];
  renderer.addEventListener(button, 'click', (event) => received.push(event));

  assert.equal(hayate.registeredListeners.length, 1);
  assert.deepEqual(hayate.registeredListeners[0], {
    elementId: 1,
    eventKind: 0,
    listenerId: 1,
  });

  // Hayate runtime bubble already resolved; host receives delivery for button listener.
  hayate.events = [[1, 0, 2, 10, 20]];
  sched.tick();

  assert.deepEqual(received, [{ kind: 'click', target: 2 }]);
});

test('removeChild clears local subtree bookkeeping', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);
  const parent = renderer.createElement('view');
  const child = renderer.createElement('view');
  const grandchild = renderer.createElement('text');

  renderer.appendChild(parent, child);
  renderer.appendChild(child, grandchild);
  renderer.addEventListener(grandchild, 'click', () => {});

  renderer.removeChild(parent, child);

  hayate.events = [[99, 0, 3, 0, 0]]; // delivery for unknown listener id must not throw
  assert.doesNotThrow(() => sched.tick());
});
