// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ACCESSKIT_ROLE_TO_ARIA,
  A11Y_ROOT_ATTR,
  A11Y_NODE_ID_PREFIX,
  MIRROR_OPACITY,
  MIRROR_POINTER_EVENTS,
  MIRROR_BOUNDS_THROTTLE_MS,
  attachAccessibilityMirror,
} from './accessibility-mirror.js';
import type { RawHayate } from './raw-hayate.js';

/**
 * Accessibility Mirror（ADR-0124）の walking skeleton 契約テスト（#592 / #645）。実 WASM を巻き込まず、
 * `poll_accessibility()` が返す AccessKit `TreeUpdate` JSON を fake `raw` で差し替え、ミラーの `poll()`
 * を手で呼んで 1 フレームずつ駆動して、生成 DOM の role / accessible name / value / 構造と、不変フレーム
 * での no-op を観測する。#645 以降、ミラーは独立 rAF ループを持たず、レンダラのフレームに相乗りして
 * `poll()` が外から駆動される（idle でレンダラが止まればミラーも完全に止まる）。
 */

/** `<canvas>` を親コンテナ配下に建てて DOM に挿す（ミラーは canvas の兄弟に建つ）。 */
function mountCanvas(): { canvas: HTMLCanvasElement; container: HTMLElement } {
  const container = document.createElement('div');
  const canvas = document.createElement('canvas');
  container.appendChild(canvas);
  document.body.appendChild(container);
  return { canvas, container };
}

/** `poll_accessibility()` だけを差し替えた最小 raw。返す JSON 文字列を後から切り替えられる。 */
function fakeRawPolling(initial: string | null): RawHayate & { json: string | null } {
  const noop = () => undefined;
  const raw = {
    json: initial,
    poll_accessibility(): string | null {
      return raw.json;
    },
  } as unknown as RawHayate & { json: string | null };
  // 触られない他メソッドは no-op で埋める。
  for (const m of [
    'element_create',
    'set_root',
    'element_append_child',
    'element_insert_before',
    'element_remove',
    'apply_mutations',
    'on_pointer_move',
    'on_pointer_down',
    'on_pointer_up',
    'on_wheel',
    'on_key_down',
    'on_text_input',
    'render',
    'set_background_color',
    'set_tuning',
  ] as const) {
    (raw as unknown as Record<string, unknown>)[m] = noop;
  }
  return raw;
}

/**
 * AccessKit `TreeUpdate`（accesskit 0.24, serde）の JSON 形を模した fixture ビルダ。
 * - `nodes`: `[NodeId(number), Node]` の配列。`Node` は `{ role, actions, childActions, flags, properties }`。
 * - `properties`: camelCase の PropertyId キー（`children` / `label` / `value` / `bounds`）。
 * - `Role`: camelCase 文字列（`window` / `button` / `textInput` / `list` / `listItem` / ...）。
 */
function node(
  role: string,
  properties: Record<string, unknown> = {},
): Record<string, unknown> {
  return { role, actions: 0, childActions: 0, flags: 0, properties };
}
function treeUpdate(
  rootId: number,
  nodes: Array<[number, Record<string, unknown>]>,
  focus = rootId,
): string {
  return JSON.stringify({
    nodes,
    tree: { root: rootId, toolkitName: null, toolkitVersion: null },
    treeId: '00000000-0000-0000-0000-000000000000',
    focus,
  });
}

/** todo を模した代表 fixture: window > [ button(Add), textInput, list > listItem ]。 */
function todoFixture(): string {
  return treeUpdate(1, [
    [1, node('window', { children: [2, 3, 4] })],
    [2, node('button', { label: 'Add', value: 'Add' })],
    [3, node('textInput', { value: 'Buy milk' })],
    [4, node('list', { children: [5] })],
    [5, node('listItem', { value: 'first todo' })],
  ]);
}

describe('attachAccessibilityMirror', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('builds a data-hayate-a11y root as the canvas sibling, invisible and non-interactive', () => {
    const { canvas, container } = mountCanvas();
    attachAccessibilityMirror(fakeRawPolling(null), canvas);

    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    expect(root).not.toBeNull();
    expect(root.parentElement).toBe(container);
    expect(root.style.opacity).toBe(MIRROR_OPACITY);
    expect(root.style.pointerEvents).toBe(MIRROR_POINTER_EVENTS);
  });

  it('clips the root to the viewport so off-canvas node bounds cannot inflate document scroll size', () => {
    // 退行防止: root が `position:absolute` のまま無制限だと、画面外/折り返し前の bounds を持つ
    // 子孫が documentElement の scrollWidth/Height を押し広げ、モバイルブラウザがレイアウト
    // ビューポートをそれに追従させて `#renderer-switch`（shell の position:fixed オーバーレイ）が
    // 実画面外へ押し出される（Canvas モードのみ・DOM モードはミラーが無く無縁）。
    const { canvas } = mountCanvas();
    attachAccessibilityMirror(fakeRawPolling(null), canvas);

    const root = document.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    expect(root.style.position).toBe('fixed');
    expect(root.style.inset).toBe('0');
    expect(root.style.overflow).toBe('hidden');
  });

  it('arms no independent frame loop: poll_accessibility is untouched until poll() is called (#645)', () => {
    const { canvas } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    const spy = vi.spyOn(raw, 'poll_accessibility');
    const mirror = attachAccessibilityMirror(raw, canvas);

    // attach 自体はレンダラフレームを掴まない。外から poll() されるまで一切走らない。
    expect(spy).not.toHaveBeenCalled();

    mirror.poll();
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('projects the TreeUpdate 1:1 with correct ARIA role, accessible name and value', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    const mirror = attachAccessibilityMirror(raw, canvas);

    mirror.poll(); // 1 フレーム poll → 投影

    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;

    // role 写像: button→button, textInput→textbox, list→list, listItem→listitem。
    const button = root.querySelector('[role="button"]') as HTMLElement;
    expect(button).not.toBeNull();
    expect(button.getAttribute('aria-label')).toBe('Add');

    const textbox = root.querySelector('[role="textbox"]') as HTMLElement;
    expect(textbox).not.toBeNull();
    expect(textbox.textContent).toBe('Buy milk');

    // 構造 1:1: list > listItem、listItem の value が textContent。
    const list = root.querySelector('[role="list"]') as HTMLElement;
    const item = list.querySelector('[role="listitem"]') as HTMLElement;
    expect(item).not.toBeNull();
    expect(item.textContent).toBe('first todo');
  });

  it('uses the role mapping table for AccessKit → ARIA role strings', () => {
    expect(ACCESSKIT_ROLE_TO_ARIA.button).toBe('button');
    expect(ACCESSKIT_ROLE_TO_ARIA.textInput).toBe('textbox');
    expect(ACCESSKIT_ROLE_TO_ARIA.list).toBe('list');
    expect(ACCESSKIT_ROLE_TO_ARIA.listItem).toBe('listitem');
  });

  it('skips DOM mutation when the polled JSON is unchanged (cheap string compare)', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    const mirror = attachAccessibilityMirror(raw, canvas);

    mirror.poll(); // 投影
    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    const button = root.querySelector('[role="button"]') as HTMLElement;

    // DOM を外から改竄しておく。JSON 不変なら 2 フレーム目は触らないので改竄が残る。
    button.setAttribute('aria-label', 'TAMPERED');
    mirror.poll(); // 同一 JSON → no-op

    expect(
      (root.querySelector('[role="button"]') as HTMLElement).getAttribute('aria-label'),
    ).toBe('TAMPERED');
  });

  it('treats a null poll (core dirty-gate: no change) as a skip that preserves the last projection', () => {
    // #642: core の dirty ゲートが変更なしフレームで `null` を返す。ミラーは文字列比較なしに
    // スキップし、直近投影の DOM をそのまま保つ（次の実変更まで触らない）。
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    const mirror = attachAccessibilityMirror(raw, canvas);

    mirror.poll(); // 初回投影
    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    const button = root.querySelector('[role="button"]') as HTMLElement;
    button.setAttribute('aria-label', 'TAMPERED');

    // core が「変更なし」を null で返すフレーム。投影は走らず改竄が残る。
    raw.json = null;
    mirror.poll();
    expect(
      (root.querySelector('[role="button"]') as HTMLElement).getAttribute('aria-label'),
    ).toBe('TAMPERED');

    // その後の実変更（非 null）は通常どおり再投影する。
    raw.json = treeUpdate(1, [
      [1, node('window', { children: [2] })],
      [2, node('button', { label: 'Renamed' })],
    ]);
    mirror.poll();
    expect(
      (root.querySelector('[role="button"]') as HTMLElement).getAttribute('aria-label'),
    ).toBe('Renamed');
  });

  it('re-projects when the polled JSON changes', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    const mirror = attachAccessibilityMirror(raw, canvas);

    mirror.poll();
    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;

    // textInput の値が変わる新フレーム。
    raw.json = treeUpdate(1, [
      [1, node('window', { children: [3] })],
      [3, node('textInput', { value: 'Buy bread' })],
    ]);
    mirror.poll();

    const textbox = root.querySelector('[role="textbox"]') as HTMLElement;
    expect(textbox.textContent).toBe('Buy bread');
  });

  it('absolutely positions each node to its on-canvas bounds rect (ADR-0124)', () => {
    const { canvas, container } = mountCanvas();
    // bounds は AccessKit Rect: {x0,y0,x1,y1}（content 絶対座標）。
    const raw = fakeRawPolling(
      treeUpdate(1, [
        [1, node('window', { children: [2] })],
        [2, node('button', { label: 'Add', bounds: { x0: 10, y0: 20, x1: 110, y1: 60 } })],
      ]),
    );
    const mirror = attachAccessibilityMirror(raw, canvas);
    mirror.poll();

    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    const button = root.querySelector('[role="button"]') as HTMLElement;
    expect(button.style.position).toBe('absolute');
    expect(button.style.left).toBe('10px');
    expect(button.style.top).toBe('20px');
    expect(button.style.width).toBe('100px');
    expect(button.style.height).toBe('40px');
  });

  it('reflects TreeUpdate.focus via aria-activedescendant on the mirror root (ADR-0124)', () => {
    const { canvas, container } = mountCanvas();
    // focus = 3（textInput）。
    const raw = fakeRawPolling(
      treeUpdate(
        1,
        [
          [1, node('window', { children: [2, 3] })],
          [2, node('button', { label: 'Add' })],
          [3, node('textInput', { value: 'Buy milk' })],
        ],
        3,
      ),
    );
    const mirror = attachAccessibilityMirror(raw, canvas);
    mirror.poll();

    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    const focusId = `${A11Y_NODE_ID_PREFIX}3`;
    expect(root.getAttribute('aria-activedescendant')).toBe(focusId);
    // 指す id を持つ要素が実在し、focus 中の textbox であること。
    const focused = container.querySelector(`#${focusId}`) as HTMLElement;
    expect(focused).not.toBeNull();
    expect(focused.getAttribute('role')).toBe('textbox');
  });

  it('moves focus reflection when TreeUpdate.focus changes', () => {
    const { canvas, container } = mountCanvas();
    const nodes: Array<[number, Record<string, unknown>]> = [
      [1, node('window', { children: [2, 3] })],
      [2, node('button', { label: 'Add' })],
      [3, node('textInput', { value: 'Buy milk' })],
    ];
    const raw = fakeRawPolling(treeUpdate(1, nodes, 3));
    const mirror = attachAccessibilityMirror(raw, canvas);
    mirror.poll();

    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    expect(root.getAttribute('aria-activedescendant')).toBe(`${A11Y_NODE_ID_PREFIX}3`);

    // focus が button(2) に移る。
    raw.json = treeUpdate(1, nodes, 2);
    mirror.poll();
    expect(root.getAttribute('aria-activedescendant')).toBe(`${A11Y_NODE_ID_PREFIX}2`);
  });

  it('detach removes the mirror root; a later poll is a safe no-op (no lingering loop, #645)', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    const mirror = attachAccessibilityMirror(raw, canvas);

    mirror.poll();
    expect(container.querySelector(`[${A11Y_ROOT_ATTR}]`)).not.toBeNull();

    mirror.detach();
    expect(container.querySelector(`[${A11Y_ROOT_ATTR}]`)).toBeNull();

    // detach 後の相乗り poll が来ても DOM を再生させず、例外も投げない。
    raw.json = treeUpdate(1, [[1, node('window', {})]]);
    expect(() => mirror.poll()).not.toThrow();
    expect(container.querySelector(`[${A11Y_ROOT_ATTR}]`)).toBeNull();
  });

  it('is a no-op when the canvas is not attached to a document (non-DOM env safety)', () => {
    const raw = fakeRawPolling(null);
    const spy = vi.spyOn(raw, 'poll_accessibility');
    const mirror = attachAccessibilityMirror(raw, {} as HTMLCanvasElement);
    expect(() => mirror.poll()).not.toThrow();
    expect(spy).not.toHaveBeenCalled();
    expect(() => mirror.detach()).not.toThrow();
  });
});

describe('attachAccessibilityMirror bounds throttle (#646)', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  /** 単一 button（window>button）を、与えた bounds / label で作る fixture。 */
  function buttonFixture(
    bounds: { x0: number; y0: number; x1: number; y1: number },
    label = 'Add',
  ): string {
    return treeUpdate(1, [
      [1, node('window', { children: [2] })],
      [2, node('button', { label, bounds })],
    ]);
  }

  /** 注入 clock 付きで attach する。`clock.t` を進めて `poll()` するとフレーム時刻を制御できる。 */
  function attachWithClock(raw: RawHayate, canvas: HTMLCanvasElement) {
    const clock = { t: 0 };
    const mirror = attachAccessibilityMirror(raw, canvas, {
      now: () => clock.t,
      boundsThrottleMs: 100,
    });
    return { mirror, clock };
  }

  function buttonEl(container: HTMLElement): HTMLElement {
    return container.querySelector('[role="button"]') as HTMLElement;
  }

  it('exposes a named, positive throttle interval constant (no magic number)', () => {
    expect(typeof MIRROR_BOUNDS_THROTTLE_MS).toBe('number');
    expect(MIRROR_BOUNDS_THROTTLE_MS).toBeGreaterThan(0);
  });

  it('throttles bounds-only DOM writes during a scroll burst (AC1: work-count)', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(buttonFixture({ x0: 0, y0: 0, x1: 100, y1: 40 }));
    const { mirror, clock } = attachWithClock(raw, canvas);

    mirror.poll(); // t=0 初回投影（構造変化）→ bounds 反映（left=0）
    expect(buttonEl(container).style.left).toBe('0px');

    // スクロール中の連続フレーム：bounds のみが毎フレーム変わる（構造・label は不変）。
    // すべて throttle 窓（100ms）内なので DOM の bounds は書き換わらない（最初の値のまま）。
    for (const [t, y] of [
      [10, 5],
      [20, 12],
      [30, 21],
    ] as const) {
      clock.t = t;
      raw.json = buttonFixture({ x0: 0, y0: y, x1: 100, y1: y + 40 });
      mirror.poll();
    }
    expect(buttonEl(container).style.top).toBe('0px'); // throttle により最新 y は未反映。
  });

  it('always reflects the final bounds after the scroll settles (AC2: no drop)', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(buttonFixture({ x0: 0, y0: 0, x1: 100, y1: 40 }));
    const { mirror, clock } = attachWithClock(raw, canvas);

    mirror.poll(); // t=0 初回

    // throttle 窓内でスクロール（bounds のみ変化）。最後のフレームの y=21 が最終値。
    for (const [t, y] of [
      [10, 5],
      [20, 12],
      [30, 21],
    ] as const) {
      clock.t = t;
      raw.json = buttonFixture({ x0: 0, y0: y, x1: 100, y1: y + 40 });
      mirror.poll();
    }
    expect(buttonEl(container).style.top).toBe('0px'); // まだ throttle 中。

    // スクロール静定：core の dirty ゲートが「変更なし」を null で返すフレーム。
    // 保留していた最終 bounds を必ず反映する（取りこぼしなし）。
    clock.t = 40;
    raw.json = null;
    mirror.poll();
    expect(buttonEl(container).style.top).toBe('21px');
  });

  it('reflects deferred bounds once the throttle window elapses during a long scroll', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(buttonFixture({ x0: 0, y0: 0, x1: 100, y1: 40 }));
    const { mirror, clock } = attachWithClock(raw, canvas);

    mirror.poll(); // t=0 apply（left/top=0）

    clock.t = 50; // 窓内
    raw.json = buttonFixture({ x0: 0, y0: 10, x1: 100, y1: 50 });
    mirror.poll();
    expect(buttonEl(container).style.top).toBe('0px'); // deferred。

    clock.t = 120; // 窓（100ms）経過
    raw.json = buttonFixture({ x0: 0, y0: 33, x1: 100, y1: 73 });
    mirror.poll();
    expect(buttonEl(container).style.top).toBe('33px'); // 窓経過で最新 bounds を反映。
  });

  it('reflects structural / label / value / focus changes immediately, even mid-throttle (AC3)', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(buttonFixture({ x0: 0, y0: 0, x1: 100, y1: 40 }, 'Add'));
    const { mirror, clock } = attachWithClock(raw, canvas);

    mirror.poll(); // t=0
    expect(buttonEl(container).getAttribute('aria-label')).toBe('Add');

    // throttle 窓内（t=10）だが label が変わる = 構造変化。遅延なく反映し、bounds も同時に更新する。
    clock.t = 10;
    raw.json = buttonFixture({ x0: 0, y0: 7, x1: 100, y1: 47 }, 'Rename');
    mirror.poll();
    expect(buttonEl(container).getAttribute('aria-label')).toBe('Rename');
    expect(buttonEl(container).style.top).toBe('7px'); // 構造変化フレームは bounds も即時。
  });
});
