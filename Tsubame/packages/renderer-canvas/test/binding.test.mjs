// T5: apply_mutations バインディング動作確認。
// 実 Hayate WASM の代わりに同形の JS スタブを差し込み、CanvasRenderer が
// 生成する ops ストリームと styles の TAG エンコーディングが契約どおりかを検証する。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CanvasRenderer, OP, TAG } from '../dist/index.js';

/** apply_mutations / element_set_text / poll_events を記録する Hayate スタブ。 */
class StubHayate {
  frames = [];
  texts = [];
  eventQueue = [];

  apply_mutations(ops, styles) {
    this.frames.push({ ops: Array.from(ops), styles: Array.from(styles) });
  }
  element_set_text(id, text) {
    this.texts.push({ id, text });
  }
  // ADR-0034: Array<Array<any>> 形式で返す（フラット Float64Array ではない）
  poll_events() {
    const e = this.eventQueue;
    this.eventQueue = [];
    // [[kind, target], [kind, target], ...] に変換
    const result = [];
    for (let i = 0; i + 1 < e.length; i += 2) {
      result.push([e[i], e[i + 1]]);
    }
    return result;
  }
}

/** 手動でフレームを進められる RAF スケジューラ。 */
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
    tick: () => {
      const cb = pending;
      pending = null;
      if (cb) cb();
    },
  };
}

test('ops ストリームと styles を 1 回/frame で集約する', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const r = new CanvasRenderer(hayate, sched);

  const a = r.createElement('view'); // id 1, kind view=0
  const b = r.createElement('text'); // id 2, kind text=1
  r.setRoot(a);
  r.appendChild(a, b);
  r.setStyle(a, { width: 100, backgroundColor: '#ff0000' });
  r.setText(b, 'Hello');

  // フラッシュ前は WASM は一切呼ばれない（境界コスト O(1)/frame）。
  assert.equal(hayate.frames.length, 0);

  sched.tick();

  assert.equal(hayate.frames.length, 1);
  const { ops, styles } = hayate.frames[0];

  // CREATE a(9,1,0), CREATE b(9,2,1), SET_ROOT(3,1), APPEND_CHILD(0,1,2), SET_STYLE(4,1,0,len)
  // styles: [WIDTH=5, 100, 0(px)] + [BACKGROUND_COLOR=0, 1,0,0,1] = 3+5 = 8 slots
  assert.deepEqual(ops, [
    OP.CREATE, 1, 0,
    OP.CREATE, 2, 1,
    OP.SET_ROOT, 1,
    OP.APPEND_CHILD, 1, 2,
    OP.SET_STYLE, 1, 0, 8,
  ]);

  // styles: [WIDTH, 100, 0(px_unit)] + [BACKGROUND_COLOR, r=1, g=0, b=0, a=1]
  assert.deepEqual(styles, [
    TAG.WIDTH, 100, 0,
    TAG.BACKGROUND_COLOR, 1, 0, 0, 1,
  ]);

  // 文字列 op はバッチ外、かつ OP_CREATE フラッシュ後に適用される。
  assert.deepEqual(hayate.texts, [{ id: 2, text: 'Hello' }]);

  assert.equal(r.constructor.name, 'CanvasRenderer');
});

test('スカラースタイルが正しくエンコードされる', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const r = new CanvasRenderer(hayate, sched);
  r.createElement('view');
  r.setStyle(1, { opacity: 0.5, borderRadius: 8 });
  sched.tick();

  const { ops, styles } = hayate.frames[0];
  // OPACITY(tag=1): [1, 0.5] = 2 slots
  // BORDER_RADIUS(tag=2): [2, 8] = 2 slots
  // style_len = 4
  assert.equal(ops[ops.length - 1], 4);
  assert.deepEqual(styles, [TAG.OPACITY, 0.5, TAG.BORDER_RADIUS, 8]);
});

test('poll_events で登録済み handler が invoke される', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const r = new CanvasRenderer(hayate, sched);
  const a = r.createElement('button'); // id 1

  const received = [];
  const unsub = r.addEventListener(a, 'click', (e) => received.push(e));

  hayate.eventQueue = [0, 1]; // click(code=0) on id 1
  sched.tick();
  assert.deepEqual(received, [{ kind: 'click', target: 1 }]);

  // 購読解除後は invoke されない。
  unsub();
  hayate.eventQueue = [0, 1];
  sched.tick();
  assert.equal(received.length, 1);
});

test('click は子要素から祖先 handler へバブリングする', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const r = new CanvasRenderer(hayate, sched);
  const button = r.createElement('button'); // id 1
  const label = r.createElement('text');   // id 2（button の子テキスト）
  r.appendChild(button, label);

  const received = [];
  r.addEventListener(button, 'click', (e) => received.push(e));

  // 子（label=2）がヒットしても、親（button=1）の handler が発火する。
  hayate.eventQueue = [0, 2];
  sched.tick();
  assert.deepEqual(received, [{ kind: 'click', target: 1 }]);
});

test('hover はバブリングしない（DOM と一致）', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const r = new CanvasRenderer(hayate, sched);
  const parent = r.createElement('view'); // id 1
  const child = r.createElement('view');  // id 2
  r.appendChild(parent, child);

  const received = [];
  r.addEventListener(parent, 'hover-enter', (e) => received.push(e));
  hayate.eventQueue = [10, 2]; // hover-enter(code=10) on child
  sched.tick();
  assert.equal(received.length, 0);
});

test('未登録 element / 未知 kind のイベントは無視される', () => {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const r = new CanvasRenderer(hayate, sched);
  r.createElement('view');
  hayate.eventQueue = [0, 999, 42, 1]; // 未登録 id, 未知 kind
  assert.doesNotThrow(() => sched.tick());
});
