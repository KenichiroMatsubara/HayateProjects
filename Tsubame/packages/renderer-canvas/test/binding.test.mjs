import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  CanvasRenderer,
  parseColor,
  encodeStylePatch,
  unsetKindsOf,
} from '../dist/index.js';

/** Records the WIT-shaped calls the real WASM would receive. */
class StubHayate {
  mutations = []; // { ops: number[], styles: number[] }
  texts = [];
  unset = [];
  renders = [];
  resizes = [];
  events = [];

  apply_mutations(ops, styles) {
    this.mutations.push({ ops: Array.from(ops), styles: Array.from(styles) });
  }
  element_set_text(id, text) {
    this.texts.push({ id, text });
  }
  element_unset_style(id, kinds) {
    this.unset.push({ id, kinds: Array.from(kinds) });
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

test('CanvasRenderer batches tree mutations into apply_mutations (ADR-0039)', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const root = renderer.createElement('view');
  const text = renderer.createElement('text');
  renderer.setRoot(root);
  renderer.appendChild(root, text);
  renderer.setStyle(root, { width: '50%', backgroundColor: '#ff0000' });
  renderer.setText(text, 'Hello'); // out-of-band string op → flushes the batch

  assert.equal(hayate.mutations.length, 1);
  // OP_CREATE=9 (view=0, text=1), OP_SET_ROOT=3, OP_APPEND_CHILD=0, OP_SET_STYLE=4
  assert.deepEqual(hayate.mutations[0].ops, [
    9, 1, 0, // create view #1
    9, 2, 1, // create text #2
    3, 1, // set_root #1
    0, 1, 2, // append #2 under #1
    4, 1, 0, 8, // set_style #1, offset 0, len 8
  ]);
  // TAG_WIDTH=5 (50, percent=1), TAG_BACKGROUND_COLOR=0 (r,g,b,a)
  assert.deepEqual(hayate.mutations[0].styles, [5, 50, 1, 0, 1, 0, 0, 1]);
  assert.deepEqual(hayate.texts, [{ id: 2, text: 'Hello' }]);

  sched.tick(33);
  assert.deepEqual(hayate.renders, [33]);
});

test('setStyle null resets route to element_unset_style with numeric codes', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const node = renderer.createElement('text');
  renderer.setStyle(node, { color: null, fontSize: null, fontFamily: null });

  // color=0, font-size=1, font-family=2
  assert.deepEqual(hayate.unset, [{ id: 1, kinds: [0, 1, 2] }]);
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

test('CanvasRenderer decodes array-of-arrays events (ADR-0034) and bubbles', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);
  const button = renderer.createElement('button');
  const label = renderer.createElement('text');
  renderer.appendChild(button, label);

  const received = [];
  renderer.addEventListener(button, 'click', (event) => received.push(event));

  hayate.events = [[0, 2, 10, 20]]; // click on id 2 → bubbles to button #1
  sched.tick();

  assert.deepEqual(received, [{ kind: 'click', target: 1 }]);
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

  hayate.events = [[0, 3, 0, 0]]; // click on a pruned id must not throw
  assert.doesNotThrow(() => sched.tick());
});
