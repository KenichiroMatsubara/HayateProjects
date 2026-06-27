import { describe, expect, it, vi } from 'vitest';
import { createHayateWebHost } from './index.js';
import type { CanvasBackend } from './resolve-backend.js';
import type { RawHayate } from './raw-hayate.js';

/**
 * Real WASM を巻き込まずに web bootstrap の配線を検証する契約テスト。WebGPU プローブと
 * backend ローダは注入 seam で差し替える（Render Host の責務 — surface 初期化・
 * capability 判定・renderer 切替 — を fake で観測する）。
 */
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
  };
}

const canvas = {} as HTMLCanvasElement;

describe('createHayateWebHost', () => {
  it('loads vello when WebGPU is available and no backend override', async () => {
    const loaded: CanvasBackend[] = [];
    const raw = fakeRaw();
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => true,
      loadBackend: async (backend) => {
        loaded.push(backend);
        return raw;
      },
    });

    expect(loaded).toEqual(['vello']);
    expect(host.raw).toBe(raw);
  });

  it('falls back to tiny-skia when WebGPU is unavailable', async () => {
    const loaded: CanvasBackend[] = [];
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async (backend) => {
        loaded.push(backend);
        return fakeRaw();
      },
    });

    expect(loaded).toEqual(['tiny-skia']);
    expect(host.raw).toBeDefined();
  });

  it('honours an explicit backend override over the probe', async () => {
    const loaded: CanvasBackend[] = [];
    await createHayateWebHost(canvas, {
      backend: 'tiny-skia',
      probeWebGPU: async () => true,
      loadBackend: async (backend) => {
        loaded.push(backend);
        return fakeRaw();
      },
    });

    expect(loaded).toEqual(['tiny-skia']);
  });

  it('passes the surface canvas to the backend loader', async () => {
    const loadBackend = vi.fn(async () => fakeRaw());
    await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend,
    });

    expect(loadBackend).toHaveBeenCalledWith('tiny-skia', canvas);
  });

  it('returns a RawHayate satisfying the HayateRenderer drive surface', async () => {
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async () => fakeRaw(),
    });

    for (const method of ['apply_mutations', 'render', 'poll_events', 'register_listener'] as const) {
      expect(typeof host.raw[method]).toBe('function');
    }
  });

  it('returns a frame-clock the composition root drives the renderer with', async () => {
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async () => fakeRaw(),
    });

    expect(typeof host.requestFrame).toBe('function');
    expect(typeof host.cancelFrame).toBe('function');
  });

  it('applies dev-only tuning to the raw renderer when provided', async () => {
    const set_tuning = vi.fn();
    await createHayateWebHost(canvas, {
      tuning: '{"scroll":1}',
      probeWebGPU: async () => false,
      loadBackend: async () => fakeRaw({ set_tuning }),
    });

    expect(set_tuning).toHaveBeenCalledWith('{"scroll":1}');
  });

  it('survives invalid tuning by falling back to compiled defaults', async () => {
    const set_tuning = vi.fn(() => {
      throw new Error('bad json');
    });
    const raw = fakeRaw({ set_tuning });

    const host = await createHayateWebHost(canvas, {
      tuning: 'not json',
      probeWebGPU: async () => false,
      loadBackend: async () => raw,
    });

    expect(host.raw).toBe(raw);
  });

  it('does not touch tuning when none is provided', async () => {
    const set_tuning = vi.fn();
    await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async () => fakeRaw({ set_tuning }),
    });

    expect(set_tuning).not.toHaveBeenCalled();
  });
});
