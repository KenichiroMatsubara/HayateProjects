import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CanvasRenderer, parseColor, stylePatchToMutation } from '../dist/index.js';

class StubHayate {
  created = [];
  roots = [];
  appended = [];
  inserted = [];
  removed = [];
  styled = [];
  unset = [];
  texts = [];
  renders = [];
  resize = [];
  events = [];

  element_create(id, kind) {
    this.created.push({ id, kind });
  }

  set_root(id) {
    this.roots.push(id);
  }

  element_append_child(parent, child) {
    this.appended.push({ parent, child });
  }

  element_insert_before(parent, child, before) {
    this.inserted.push({ parent, child, before });
  }

  element_remove(id) {
    this.removed.push(id);
  }

  element_set_style(id, props) {
    this.styled.push({ id, props });
  }

  element_unset_style(id, kinds) {
    this.unset.push({ id, kinds });
  }

  element_set_text(id, text) {
    this.texts.push({ id, text });
  }

  on_resize(width, height) {
    this.resize.push({ width, height });
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

test('CanvasRenderer uses WIT-shaped APIs instead of op/tag packets', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);

  const root = renderer.createElement('view');
  const text = renderer.createElement('text');
  renderer.setRoot(root);
  renderer.appendChild(root, text);
  renderer.setStyle(root, { width: '50%', backgroundColor: '#ff0000' });
  renderer.setText(text, 'Hello');

  assert.deepEqual(hayate.created, [
    { id: 1, kind: 'view' },
    { id: 2, kind: 'text' },
  ]);
  assert.deepEqual(hayate.roots, [1]);
  assert.deepEqual(hayate.appended, [{ parent: 1, child: 2 }]);
  assert.deepEqual(hayate.styled, [{
    id: 1,
    props: [
      { width: { value: 50, unit: 'percent' } },
      { 'background-color': { r: 1, g: 0, b: 0, a: 1 } },
    ],
  }]);
  assert.deepEqual(hayate.texts, [{ id: 2, text: 'Hello' }]);

  sched.tick(33);
  assert.deepEqual(hayate.renders, [33]);
});

test('stylePatchToMutation rejects properties missing from WIT', () => {
  assert.throws(
    () => stylePatchToMutation({ fontWeight: 700 }),
    /not defined in WIT/,
  );
  assert.throws(
    () => stylePatchToMutation({ flexGrow: 1 }),
    /not defined in WIT/,
  );
});

test('stylePatchToMutation produces WIT unset kinds only', () => {
  assert.deepEqual(stylePatchToMutation({
    color: null,
    fontSize: null,
    fontFamily: null,
  }), {
    props: [],
    unsetKinds: ['color', 'font-size', 'font-family'],
  });
});

test('parseColor keeps WIT color records explicit', () => {
  assert.deepEqual(parseColor('#112233'), {
    r: 0x11 / 255,
    g: 0x22 / 255,
    b: 0x33 / 255,
    a: 1,
  });
});

test('CanvasRenderer dispatches WIT-shaped events', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(hayate, sched);
  const button = renderer.createElement('button');
  const label = renderer.createElement('text');
  renderer.appendChild(button, label);

  const received = [];
  renderer.addEventListener(button, 'click', (event) => received.push(event));

  hayate.events = [{ type: 'click', target: 2, x: 10, y: 20 }];
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

  assert.deepEqual(hayate.removed, [2]);
  hayate.events = [{ type: 'click', target: 3, x: 0, y: 0 }];
  assert.doesNotThrow(() => sched.tick());
});
