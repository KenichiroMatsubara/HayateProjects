// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  WORKER_ENGINE_QUERY_PARAM,
  WORKER_ENGINE_QUERY_VALUE,
  shouldUseWorkerEngine,
  bootWorkerEngineBridge,
  type WorkerTransport,
} from './worker-boot.js';
import type { MainToWorker, WorkerToMain, MainEditContextSink } from './worker-host.js';

/**
 * OffscreenCanvas＋単一 Worker への opt-in 配線（ADR-0128 web 半分・#648）の契約テスト。実 Worker /
 * OffscreenCanvas を巻き込まず、transport（postMessage）と canvas transfer を注入 seam で差し替えて、
 * main→Worker の input/IME 橋渡しとライフサイクル（init transfer・detach terminate）を観測する。
 */

/** postMessage を配列に貯める注入 transport。Worker→main は `emit()` で手動注入する。 */
function fakeTransport() {
  const sent: Array<{ msg: MainToWorker; transfer?: Transferable[] }> = [];
  let onMsg: ((m: WorkerToMain) => void) | null = null;
  let terminated = false;
  const transport: WorkerTransport = {
    postMessage: (msg, transfer) => sent.push({ msg, transfer }),
    onMessage: (cb) => {
      onMsg = cb;
    },
    terminate: () => {
      terminated = true;
    },
  };
  return {
    transport,
    sent,
    emit: (m: WorkerToMain) => onMsg?.(m),
    get terminated() {
      return terminated;
    },
  };
}

function mountCanvas(): HTMLCanvasElement {
  const container = document.createElement('div');
  const canvas = document.createElement('canvas');
  canvas.width = 800;
  canvas.height = 600;
  container.appendChild(canvas);
  document.body.appendChild(container);
  return canvas;
}

function recordingImeSink(): MainEditContextSink & {
  keyboardVisible: boolean;
  caretRect: unknown;
} {
  return {
    keyboardVisible: false,
    caretRect: null,
    setKeyboardVisible(v) {
      this.keyboardVisible = v;
    },
    setCaretRect(r) {
      this.caretRect = r;
    },
  };
}

describe('shouldUseWorkerEngine (opt-in gate, #648)', () => {
  it('defaults to the main-thread path when nothing opts in', () => {
    expect(shouldUseWorkerEngine(undefined, undefined)).toBe(false);
    expect(shouldUseWorkerEngine(undefined, '')).toBe(false);
    expect(shouldUseWorkerEngine(undefined, '?foo=bar')).toBe(false);
  });

  it('honours an explicit boolean flag over the query string', () => {
    expect(shouldUseWorkerEngine(true, undefined)).toBe(true);
    expect(shouldUseWorkerEngine(false, `?${WORKER_ENGINE_QUERY_PARAM}=${WORKER_ENGINE_QUERY_VALUE}`)).toBe(
      false,
    );
  });

  it('opts in via the named query parameter', () => {
    expect(
      shouldUseWorkerEngine(undefined, `?${WORKER_ENGINE_QUERY_PARAM}=${WORKER_ENGINE_QUERY_VALUE}`),
    ).toBe(true);
    expect(shouldUseWorkerEngine(undefined, `?${WORKER_ENGINE_QUERY_PARAM}=off`)).toBe(false);
  });
});

describe('bootWorkerEngineBridge (main<->worker wiring, #648)', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('transfers the OffscreenCanvas and boots the worker engine with surface metrics', () => {
    const canvas = mountCanvas();
    const t = fakeTransport();
    const offscreen = { token: 'offscreen' };
    const transferControlToOffscreen = vi.fn(() => offscreen);

    bootWorkerEngineBridge(canvas, {
      transport: t.transport,
      ime: recordingImeSink(),
      transferControlToOffscreen,
      dpr: 2,
    });

    expect(transferControlToOffscreen).toHaveBeenCalledWith(canvas);
    const init = t.sent.find((s) => s.msg.kind === 'init');
    expect(init?.msg).toEqual({ kind: 'init', canvas: offscreen, width: 800, height: 600, dpr: 2 });
    // OffscreenCanvas は transfer リストで渡す（COOP/COEP 不要）。
    expect(init?.transfer).toContain(offscreen);
  });

  it('forwards main-thread pointer / wheel / keyboard input to the worker', () => {
    const canvas = mountCanvas();
    const t = fakeTransport();
    bootWorkerEngineBridge(canvas, {
      transport: t.transport,
      ime: recordingImeSink(),
      transferControlToOffscreen: () => ({}),
      dpr: 1,
    });

    canvas.dispatchEvent(
      new PointerEvent('pointerdown', { clientX: 10, clientY: 20, bubbles: true }),
    );
    canvas.dispatchEvent(new WheelEvent('wheel', { deltaX: 0, deltaY: -120, bubbles: true }));
    globalThis.dispatchEvent(new KeyboardEvent('keydown', { key: 'a' }));

    const kinds = t.sent.map((s) => s.msg.kind);
    expect(kinds).toContain('pointer');
    expect(kinds).toContain('wheel');
    expect(kinds).toContain('key');
    const pointer = t.sent.find((s) => s.msg.kind === 'pointer')!.msg as Extract<
      MainToWorker,
      { kind: 'pointer' }
    >;
    expect(pointer.action).toBe('down');
  });

  it('applies IME presentation from the worker to the main EditContext sink (ADR-0069)', () => {
    const canvas = mountCanvas();
    const t = fakeTransport();
    const ime = recordingImeSink();
    bootWorkerEngineBridge(canvas, {
      transport: t.transport,
      ime,
      transferControlToOffscreen: () => ({}),
      dpr: 1,
    });

    t.emit({
      kind: 'ime',
      presentation: { keyboardVisible: true, caretRect: { x: 1, y: 2, width: 3, height: 4 } },
    });

    expect(ime.keyboardVisible).toBe(true);
    expect(ime.caretRect).toEqual({ x: 1, y: 2, width: 3, height: 4 });
  });

  it('detach terminates the worker and stops forwarding input (safe teardown / rebuild)', () => {
    const canvas = mountCanvas();
    const t = fakeTransport();
    const handle = bootWorkerEngineBridge(canvas, {
      transport: t.transport,
      ime: recordingImeSink(),
      transferControlToOffscreen: () => ({}),
      dpr: 1,
    });

    handle.detach();
    expect(t.terminated).toBe(true);

    const before = t.sent.length;
    // detach 後の DOM 入力はもう Worker へ流れない（リスナ除去）。
    canvas.dispatchEvent(new PointerEvent('pointerdown', { clientX: 1, clientY: 1, bubbles: true }));
    expect(t.sent.length).toBe(before);
  });
});
