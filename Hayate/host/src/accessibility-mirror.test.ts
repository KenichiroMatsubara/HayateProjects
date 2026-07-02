// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  ACCESSKIT_ROLE_TO_ARIA,
  A11Y_ROOT_ATTR,
  A11Y_NODE_ID_PREFIX,
  MIRROR_OPACITY,
  MIRROR_POINTER_EVENTS,
  attachAccessibilityMirror,
} from './accessibility-mirror.js';
import type { RawHayate } from './raw-hayate.js';

/**
 * Accessibility Mirror（ADR-0124）の walking skeleton 契約テスト（#592）。実 WASM を巻き込まず、
 * `poll_accessibility()` が返す AccessKit `TreeUpdate` JSON を fake `raw` で差し替え、rAF も注入
 * seam で 1 フレームずつ手動駆動して、生成 DOM の role / accessible name / value / 構造と、
 * 不変フレームでの no-op を観測する。fixture の作り方は `golden-frame.ts`、happy-dom 環境は
 * `renderer-dom` の prior art に倣う。
 */

/** `<canvas>` を親コンテナ配下に建てて DOM に挿す（ミラーは canvas の兄弟に建つ）。 */
function mountCanvas(): { canvas: HTMLCanvasElement; container: HTMLElement } {
  const container = document.createElement('div');
  const canvas = document.createElement('canvas');
  container.appendChild(canvas);
  document.body.appendChild(container);
  return { canvas, container };
}

/** rAF を手で進めるための駆動子。`tick()` が登録済みフレームコールバックを 1 回走らせる。 */
function frameDriver() {
  let cb: FrameRequestCallback | null = null;
  return {
    requestFrame: (fn: FrameRequestCallback): number => {
      cb = fn;
      return 1;
    },
    cancelFrame: (): void => {
      cb = null;
    },
    tick(): void {
      const fn = cb;
      cb = null;
      fn?.(0);
    },
    get scheduled(): boolean {
      return cb != null;
    },
  };
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
  let driver: ReturnType<typeof frameDriver>;

  beforeEach(() => {
    driver = frameDriver();
  });
  afterEach(() => {
    document.body.innerHTML = '';
  });

  function attach(raw: RawHayate, canvas: HTMLCanvasElement) {
    return attachAccessibilityMirror(raw, canvas, {
      requestFrame: driver.requestFrame,
      cancelFrame: driver.cancelFrame,
    });
  }

  it('builds a data-hayate-a11y root as the canvas sibling, invisible and non-interactive', () => {
    const { canvas, container } = mountCanvas();
    attach(fakeRawPolling(null), canvas);

    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    expect(root).not.toBeNull();
    expect(root.parentElement).toBe(container);
    expect(root.style.opacity).toBe(MIRROR_OPACITY);
    expect(root.style.pointerEvents).toBe(MIRROR_POINTER_EVENTS);
  });

  it('projects the TreeUpdate 1:1 with correct ARIA role, accessible name and value', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    attach(raw, canvas);

    driver.tick(); // 1 フレーム poll → 投影

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
    attach(raw, canvas);

    driver.tick(); // 投影
    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    const button = root.querySelector('[role="button"]') as HTMLElement;

    // DOM を外から改竄しておく。JSON 不変なら 2 フレーム目は触らないので改竄が残る。
    button.setAttribute('aria-label', 'TAMPERED');
    driver.tick(); // 同一 JSON → no-op

    expect(
      (root.querySelector('[role="button"]') as HTMLElement).getAttribute('aria-label'),
    ).toBe('TAMPERED');
  });

  it('treats a null poll (core dirty-gate: no change) as a skip that preserves the last projection', () => {
    // #642: core の dirty ゲートが変更なしフレームで `null` を返す。ミラーは文字列比較なしに
    // スキップし、直近投影の DOM をそのまま保つ（次の実変更まで触らない）。
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    attach(raw, canvas);

    driver.tick(); // 初回投影
    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    const button = root.querySelector('[role="button"]') as HTMLElement;
    button.setAttribute('aria-label', 'TAMPERED');

    // core が「変更なし」を null で返すフレーム。投影は走らず改竄が残る。
    raw.json = null;
    driver.tick();
    expect(
      (root.querySelector('[role="button"]') as HTMLElement).getAttribute('aria-label'),
    ).toBe('TAMPERED');

    // その後の実変更（非 null）は通常どおり再投影する。
    raw.json = treeUpdate(1, [
      [1, node('window', { children: [2] })],
      [2, node('button', { label: 'Renamed' })],
    ]);
    driver.tick();
    expect(
      (root.querySelector('[role="button"]') as HTMLElement).getAttribute('aria-label'),
    ).toBe('Renamed');
  });

  it('re-projects when the polled JSON changes', () => {
    const { canvas, container } = mountCanvas();
    const raw = fakeRawPolling(todoFixture());
    attach(raw, canvas);

    driver.tick();
    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;

    // textInput の値が変わる新フレーム。
    raw.json = treeUpdate(1, [
      [1, node('window', { children: [3] })],
      [3, node('textInput', { value: 'Buy bread' })],
    ]);
    driver.tick();

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
    attach(raw, canvas);
    driver.tick();

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
    attach(raw, canvas);
    driver.tick();

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
    attach(raw, canvas);
    driver.tick();

    const root = container.querySelector(`[${A11Y_ROOT_ATTR}]`) as HTMLElement;
    expect(root.getAttribute('aria-activedescendant')).toBe(`${A11Y_NODE_ID_PREFIX}3`);

    // focus が button(2) に移る。
    raw.json = treeUpdate(1, nodes, 2);
    driver.tick();
    expect(root.getAttribute('aria-activedescendant')).toBe(`${A11Y_NODE_ID_PREFIX}2`);
  });

  it('detach removes the mirror root and stops the rAF loop', () => {
    const { canvas, container } = mountCanvas();
    const detach = attach(fakeRawPolling(todoFixture()), canvas);

    driver.tick();
    expect(container.querySelector(`[${A11Y_ROOT_ATTR}]`)).not.toBeNull();
    expect(driver.scheduled).toBe(true);

    detach();
    expect(container.querySelector(`[${A11Y_ROOT_ATTR}]`)).toBeNull();
    expect(driver.scheduled).toBe(false);
  });

  it('is a no-op when the canvas is not attached to a document (non-DOM env safety)', () => {
    const detach = attachAccessibilityMirror(fakeRawPolling(null), {} as HTMLCanvasElement, {
      requestFrame: driver.requestFrame,
      cancelFrame: driver.cancelFrame,
    });
    expect(driver.scheduled).toBe(false);
    expect(() => detach()).not.toThrow();
  });
});
