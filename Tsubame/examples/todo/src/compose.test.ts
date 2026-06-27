import { describe, expect, it, vi } from 'vitest';
import type { IRenderer } from '@tsubame/renderer-protocol';
import type { RawHayate } from '@tsubame/renderer-hayate';
import { mountCanvasApp, type CanvasHost } from './compose.js';

// #477 wiring seam: browser (`main.tsx`) と native (`main.android.tsx`) は同一形
// 「host→raw(+clock)→HayateRenderer→mount」に縮約される。その合成ルートを注入 host
// fake で固定し、host が確立した raw と frame-clock だけで renderer が駆動されること
// （Tsubame は host を知らない）を保証する。

function fakeRaw(overrides: Partial<RawHayate> = {}): RawHayate {
  const noop = () => undefined;
  return {
    element_create: noop,
    set_root: noop,
    element_append_child: noop,
    element_insert_before: noop,
    element_remove: noop,
    element_get_text: () => '',
    element_get_bounds: () => [0, 0, 0, 0],
    element_subtree_ids: () => new Float64Array(),
    apply_mutations: noop,
    on_pointer_move: noop,
    on_pointer_down: noop,
    on_pointer_up: noop,
    on_wheel: noop,
    on_key_down: noop,
    has_selection: () => false,
    on_text_input: noop,
    poll_accessibility: () => null,
    render: noop,
    poll_events: () => [],
    register_listener: () => 0,
    set_background_color: noop,
    set_tuning: noop,
    element_effective_visual: () => null,
    ...overrides,
  } as RawHayate;
}

/** 最新フレームコールバックを保持するだけの注入 clock（native pump と同型）。 */
function fakeHost(raw: RawHayate): CanvasHost & { tick(ts?: number): void } {
  let pending: FrameRequestCallback | null = null;
  return {
    raw,
    requestFrame: vi.fn((cb: FrameRequestCallback) => {
      pending = cb;
      return 1;
    }),
    cancelFrame: vi.fn(() => {
      pending = null;
    }),
    tick(ts = 16) {
      const cb = pending;
      pending = null;
      cb?.(ts);
    },
  };
}

describe('mountCanvasApp', () => {
  it('mounts the app against a renderer built from the host', () => {
    const host = fakeHost(fakeRaw());
    const mount = vi.fn();

    mountCanvasApp(host, mount);

    expect(mount).toHaveBeenCalledTimes(1);
    const renderer = mount.mock.calls[0]![0] as IRenderer;
    expect(typeof renderer.createElement).toBe('function');
  });

  it("arms the host's frame-clock by starting the renderer", () => {
    const host = fakeHost(fakeRaw());

    mountCanvasApp(host, () => undefined);

    expect(host.requestFrame).toHaveBeenCalledTimes(1);
  });

  it('drives the injected raw through the injected clock on each tick', () => {
    const render = vi.fn();
    const host = fakeHost(fakeRaw({ render }));

    mountCanvasApp(host, () => undefined);
    expect(render).not.toHaveBeenCalled();

    host.tick(33);
    expect(render).toHaveBeenCalledWith(33);
  });

  it('returns the renderer so the native pump can stop it', () => {
    const host = fakeHost(fakeRaw());

    const renderer = mountCanvasApp(host, () => undefined);

    expect(typeof renderer.stop).toBe('function');
    renderer.stop();
    expect(host.cancelFrame).toHaveBeenCalled();
  });
});
