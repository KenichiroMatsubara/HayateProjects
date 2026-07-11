import { describe, expect, it, vi } from 'vitest';
import { Paint, Path } from '@torimi/tsubame-protocol-generated/recorder';
import type { DrawCanvas, DrawSize } from '@torimi/tsubame-renderer-protocol';
import { DomRenderer } from './dom-renderer.js';
import { DRAW_OVERFLOW_VISIBLE_MARGIN_PX } from './draw-surface.js';

// #731: DOM Renderer の draw 対応。draw 付き view に <canvas> を敷き、同一 painter
// を 2D コンテキストへ replay する（Tsubame ADR-0014・wire は通らない）。レイアウト
// サイズは注入した observeElementSize seam から届く（ブラウザ実装は ResizeObserver。
// Host 結合原則の弱形: 受け取る、掴みに行かない — happy-dom はレイアウトを持たない
// ため、テストは seam からサイズを直接発火する）。

function fillBox(canvas: DrawCanvas, size: DrawSize): void {
  canvas.drawPath(new Path().addRect(0, 0, size.width, size.height), new Paint());
}

/** 観測対象と発火口を公開する observeElementSize のテストダブル。 */
function manualSizeObserver() {
  const observed = new Map<HTMLElement, (size: DrawSize) => void>();
  const disconnects: HTMLElement[] = [];
  return {
    observed,
    disconnects,
    observeElementSize: (target: HTMLElement, onSize: (size: DrawSize) => void) => {
      observed.set(target, onSize);
      return () => {
        observed.delete(target);
        disconnects.push(target);
      };
    },
    fire(target: HTMLElement, size: DrawSize): void {
      observed.get(target)?.(size);
    },
  };
}

/** 2D コンテキストの呼び出しを [name, ...args] で記録するモック。 */
function recordingContext() {
  const calls: Array<[string, ...unknown[]]> = [];
  const handler: ProxyHandler<Record<string, unknown>> = {
    get(_t, prop: string) {
      return (...args: unknown[]) => {
        calls.push([prop, ...args]);
      };
    },
    set(_t, prop: string, value) {
      calls.push([`set ${prop}`, value]);
      return true;
    },
  };
  return { ctx: new Proxy({}, handler), calls };
}

function setup() {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const sizes = manualSizeObserver();
  const renderer = new DomRenderer({
    container,
    document,
    observeElementSize: sizes.observeElementSize,
  });
  const view = renderer.createElement('view');
  renderer.setRoot(view);
  const viewEl = container.querySelector('[data-tsubame-id]') as HTMLElement;
  return { renderer, view, viewEl, sizes };
}

/** renderer が敷いた draw 用 canvas。 */
function drawCanvasOf(viewEl: HTMLElement): HTMLCanvasElement | null {
  return viewEl.querySelector('canvas');
}

describe('DomRenderer draw property (#731)', () => {
  it('lays a non-interactive <canvas> under the view as its first child and observes its size', () => {
    const { renderer, view, viewEl, sizes } = setup();
    renderer.appendChild(view, renderer.createElement('text'));

    renderer.setDraw(view, fillBox);

    const canvas = drawCanvasOf(viewEl)!;
    expect(canvas).not.toBeNull();
    // 描画順 background → border → draw → children: canvas は先頭（子より下）。
    expect(viewEl.firstElementChild).toBe(canvas);
    // hit-test は box 判定のまま（ADR-0141）: canvas は入力を拾わない。
    expect(canvas.style.pointerEvents).toBe('none');
    expect(canvas.style.position).toBe('absolute');
    // レイアウトサイズの観測が始まっている。
    expect(sizes.observed.has(viewEl)).toBe(true);
  });

  it('sizes the canvas past the box by the named margin under overflow: visible and replays the painter', () => {
    const { renderer, view, viewEl, sizes } = setup();
    renderer.setDraw(view, fillBox);
    const canvas = drawCanvasOf(viewEl)!;
    const { ctx, calls } = recordingContext();
    canvas.getContext = (() => ctx) as unknown as typeof canvas.getContext;

    sizes.fire(viewEl, { width: 100, height: 50 });

    const m = DRAW_OVERFLOW_VISIBLE_MARGIN_PX;
    // box 外へ margin ぶん張り出す = overflow: visible ではみ出しが切れない。
    expect(canvas.style.left).toBe(`${-m}px`);
    expect(canvas.style.top).toBe(`${-m}px`);
    expect(canvas.style.width).toBe(`${100 + 2 * m}px`);
    expect(canvas.style.height).toBe(`${50 + 2 * m}px`);
    // happy-dom の devicePixelRatio は 1: 物理解像度 = css サイズ。
    expect(canvas.width).toBe(100 + 2 * m);
    expect(canvas.height).toBe(50 + 2 * m);
    // クリアして全 replay。painter 原点は box 左上（margin ぶん平行移動）。
    expect(calls[0]).toEqual(['setTransform', 1, 0, 0, 1, 0, 0]);
    expect(calls[1]).toEqual(['clearRect', 0, 0, 100 + 2 * m, 50 + 2 * m]);
    expect(calls[2]).toEqual(['setTransform', 1, 0, 0, 1, m, m]);
    expect(calls).toContainEqual(['rect', 0, 0, 100, 50]);
    expect(calls).toContainEqual(['fill', 'nonzero']);
  });

  it('sizes the canvas exactly to the box under overflow: hidden (CSS clips at the box)', () => {
    const { renderer, view, viewEl, sizes } = setup();
    renderer.setStyle(view, { overflow: 'hidden' });
    renderer.setDraw(view, fillBox);
    const canvas = drawCanvasOf(viewEl)!;
    const { ctx } = recordingContext();
    canvas.getContext = (() => ctx) as unknown as typeof canvas.getContext;

    sizes.fire(viewEl, { width: 100, height: 50 });

    // 親の overflow: hidden が box でクリップするので、canvas は box ぴったり。
    expect(viewEl.style.overflow).toBe('hidden');
    expect(canvas.style.left).toBe('0px');
    expect(canvas.style.top).toBe('0px');
    expect(canvas.style.width).toBe('100px');
    expect(canvas.style.height).toBe('50px');
  });

  it('skips the re-replay when shouldRepaint(old) is false, replays when true', () => {
    const { renderer, view, viewEl, sizes } = setup();
    const first = { paint: vi.fn(fillBox) };
    renderer.setDraw(view, first);
    const canvas = drawCanvasOf(viewEl)!;
    const { ctx } = recordingContext();
    canvas.getContext = (() => ctx) as unknown as typeof canvas.getContext;
    sizes.fire(viewEl, { width: 100, height: 50 });
    expect(first.paint).toHaveBeenCalledTimes(1);

    const declined = { paint: vi.fn(fillBox), shouldRepaint: () => false };
    renderer.setDraw(view, declined);
    expect(declined.paint).not.toHaveBeenCalled();

    const accepted = { paint: vi.fn(fillBox), shouldRepaint: () => true };
    renderer.setDraw(view, accepted);
    expect(accepted.paint).toHaveBeenCalledTimes(1);
    expect(accepted.paint).toHaveBeenCalledWith(expect.anything(), { width: 100, height: 50 });
  });

  it('stops observing and drops draw state when the element leaves the tree', () => {
    const { renderer, view, viewEl, sizes } = setup();
    const child = renderer.createElement('view');
    renderer.appendChild(view, child);
    const childEl = viewEl.querySelector('[data-tsubame-id="2"]') as HTMLElement;
    renderer.setDraw(child, fillBox);
    expect(sizes.observed.has(childEl)).toBe(true);

    renderer.removeChild(view, child);

    // 観測が生き残ると removeChild 後も observer と canvas がリークする。
    expect(sizes.observed.has(childEl)).toBe(false);
    expect(sizes.disconnects).toEqual([childEl]);
  });

  it('removes the canvas and stops observing when draw is set to null', () => {
    const { renderer, view, viewEl, sizes } = setup();
    renderer.setDraw(view, fillBox);
    expect(drawCanvasOf(viewEl)).not.toBeNull();

    renderer.setDraw(view, null);

    expect(drawCanvasOf(viewEl)).toBeNull();
    expect(sizes.observed.has(viewEl)).toBe(false);
    expect(sizes.disconnects).toEqual([viewEl]);
  });
});
