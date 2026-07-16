import { describe, expect, it, vi } from 'vitest';
import {
  createHayateWebHost,
  WORKER_ENGINE_QUERY_PARAM,
  WORKER_ENGINE_QUERY_VALUE,
} from './index.js';
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
    dispatch_edit_intent: () => 1,
    has_selection: () => false,
    on_text_input: noop,
    poll_accessibility: () => null,
    render: noop,
    prepare_frame: () => [1],
    commit_frame: noop,
    abort_frame: noop,
    has_pending_visual_work: () => false,
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
  it('tries CanvasKit first, then advances through unselected boot candidates after init failure', async () => {
    const loaded: CanvasBackend[] = [];
    const raw = fakeRaw();

    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => true,
      loadBackend: async (backend) => {
        loaded.push(backend);
        if (backend === 'canvaskit') throw new Error('CanvasKit surface unavailable');
        return raw;
      },
    });

    expect(loaded).toEqual(['canvaskit', 'vello']);
    expect(host.raw).toBe(raw);
  });

  it('loads CanvasKit first when no backend override is present', async () => {
    const loaded: CanvasBackend[] = [];
    const raw = fakeRaw();
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => true,
      loadBackend: async (backend) => {
        loaded.push(backend);
        return raw;
      },
    });

    expect(loaded).toEqual(['canvaskit']);
    expect(host.raw).toBe(raw);
  });

  it('still starts with CanvasKit when WebGPU is unavailable', async () => {
    const loaded: CanvasBackend[] = [];
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async (backend) => {
        loaded.push(backend);
        return fakeRaw();
      },
    });

    expect(loaded).toEqual(['canvaskit']);
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

  it('honours a ?renderer= deep link over the WebGPU probe (am start -e equivalent)', async () => {
    const loaded: CanvasBackend[] = [];
    await createHayateWebHost(canvas, {
      // WebGPU があっても、ディープリンクの強制指定が勝つ。detectMode を通さず host を
      // 直接叩く呼び出し元（deep link）でも tiny-skia を選べる。
      locationSearch: '?renderer=tiny-skia',
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

    expect(loadBackend).toHaveBeenCalledWith('canvaskit', canvas);
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

  it('attaches the accessibility mirror seam with the surface raw+canvas (ADR-0124)', async () => {
    const attachMirror = vi.fn(() => ({ poll: () => {}, detach: () => {} }));
    const raw = fakeRaw();
    await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async () => raw,
      attachMirror,
    });

    expect(attachMirror).toHaveBeenCalledWith(raw, canvas);
  });

  it('exposes the mirror detach as the host teardown seam (full reload calls it)', async () => {
    const detach = vi.fn();
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async () => fakeRaw(),
      attachMirror: () => ({ poll: () => {}, detach }),
    });

    host.detach();

    expect(detach).toHaveBeenCalledTimes(1);
  });

  it('rides the mirror poll on the renderer frame-clock: polls once after each frame (#645)', async () => {
    // レンダラのフレーム cb → その末尾でミラー poll、の順で 1 フレームにつき 1 回だけ相乗りする。
    let scheduled: FrameRequestCallback | null = null;
    const poll = vi.fn();
    const host = await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async () => fakeRaw(),
      attachMirror: () => ({ poll, detach: () => {} }),
      requestFrame: (cb) => {
        scheduled = cb;
        return 1;
      },
      cancelFrame: () => {},
    });

    const order: string[] = [];
    // 合成ルート（HayateRenderer 相当）が host.requestFrame でフレームを予約する。
    host.requestFrame(() => order.push('render'));
    expect(poll).not.toHaveBeenCalled(); // 予約だけ。まだフレームは出ていない。

    // clock が 1 フレーム発火する。
    scheduled!(0);
    order.push('poll:' + poll.mock.calls.length);
    expect(order).toEqual(['render', 'poll:1']); // レンダラ → ミラー poll の順。
    expect(poll).toHaveBeenCalledTimes(1);
  });

  it('does not tick the mirror while idle: no scheduled frame means no poll (#645)', async () => {
    // フレームが予約されない（visual_dirty 空・入力なし）限りミラー poll はゼロ。独立ループが無い。
    const poll = vi.fn();
    await createHayateWebHost(canvas, {
      probeWebGPU: async () => false,
      loadBackend: async () => fakeRaw(),
      attachMirror: () => ({ poll, detach: () => {} }),
      requestFrame: () => 1,
      cancelFrame: () => {},
    });

    // host は attach しただけでフレームを予約していない。ミラーは一切走らない。
    expect(poll).not.toHaveBeenCalled();
  });

  // ── OffscreenCanvas + Worker opt-in（ADR-0128 web 半分・#648）──────────────────
  function fakeWorkerTransport() {
    const sent: unknown[] = [];
    return {
      transport: {
        postMessage: (msg: unknown) => sent.push(msg),
        onMessage: () => {},
        terminate: vi.fn(),
      },
      sent,
    };
  }
  const workerCanvas = {
    width: 800,
    height: 600,
    addEventListener: () => {},
    removeEventListener: () => {},
  } as unknown as HTMLCanvasElement;

  it('opt-in off (default): loads a main-thread backend, spawns no worker (#648)', async () => {
    const spawnWorker = vi.fn();
    const loadBackend = vi.fn(async () => fakeRaw());
    await createHayateWebHost(workerCanvas, {
      probeWebGPU: async () => false,
      loadBackend,
      spawnWorker,
      locationSearch: '',
    });

    expect(loadBackend).toHaveBeenCalledTimes(1);
    expect(spawnWorker).not.toHaveBeenCalled();
  });

  it('opt-in on (flag): boots the worker engine and does not load a main-thread backend (#648)', async () => {
    const { transport } = fakeWorkerTransport();
    const spawnWorker = vi.fn(() => transport);
    const loadBackend = vi.fn(async () => fakeRaw());
    const transferControlToOffscreen = vi.fn(() => ({ token: 'offscreen' }));

    const host = await createHayateWebHost(workerCanvas, {
      workerEngine: true,
      spawnWorker,
      transferControlToOffscreen,
      loadBackend,
    });

    // Worker がエンジンを所有：main では WASM backend をロードしない（毎フレームのエンジン仕事が無い）。
    expect(loadBackend).not.toHaveBeenCalled();
    expect(spawnWorker).toHaveBeenCalledTimes(1);
    expect(transferControlToOffscreen).toHaveBeenCalledWith(workerCanvas);
    // main の raw は input proxy：入力面は関数、drive/query は不活性の既定。
    expect(typeof host.raw.on_pointer_down).toBe('function');
    expect(host.raw.poll_events()).toEqual([]);
  });

  it('opt-in on (query param): boots the worker path (#648)', async () => {
    const { transport } = fakeWorkerTransport();
    const spawnWorker = vi.fn(() => transport);
    await createHayateWebHost(workerCanvas, {
      spawnWorker,
      transferControlToOffscreen: () => ({}),
      locationSearch: `?${WORKER_ENGINE_QUERY_PARAM}=${WORKER_ENGINE_QUERY_VALUE}`,
      loadBackend: async () => fakeRaw(),
    });

    expect(spawnWorker).toHaveBeenCalledTimes(1);
  });

  it('opt-in on but no spawnWorker: falls back to the main-thread path (#648)', async () => {
    const loadBackend = vi.fn(async () => fakeRaw());
    const host = await createHayateWebHost(workerCanvas, {
      workerEngine: true,
      probeWebGPU: async () => false,
      loadBackend,
    });

    expect(loadBackend).toHaveBeenCalledTimes(1);
    expect(host.raw).toBeDefined();
  });

  it('worker host detach terminates the worker (safe teardown / rebuild, #648)', async () => {
    const { transport } = fakeWorkerTransport();
    const host = await createHayateWebHost(workerCanvas, {
      workerEngine: true,
      spawnWorker: () => transport,
      transferControlToOffscreen: () => ({}),
    });

    host.detach();
    expect(transport.terminate).toHaveBeenCalledTimes(1);
  });
});
