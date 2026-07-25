// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  bootWorkerEngineBridge,
  createWorkerInputProxy,
  workerSurfaceMetrics,
  type WorkerTransport,
} from './worker-boot.js';
import type { MainToWorker, WorkerToMain, MainEditContextSink } from './worker-host.js';

/**
 * Canvas Mode の標準 OffscreenCanvas＋単一 Worker 配線（ADR-0128 / ADR-0157）の契約テスト。実 Worker /
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

describe('worker surface metrics', () => {
  it('converts CSS pixels to a DPR-scaled OffscreenCanvas buffer without zero dimensions', () => {
    expect(workerSurfaceMetrics(320, 180, 2)).toEqual({
      width: 640,
      height: 360,
      dpr: 2,
    });
    expect(workerSurfaceMetrics(0, 0, 0)).toEqual({
      width: 1,
      height: 1,
      dpr: 1,
    });
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
    expect(canvas.style.touchAction).toBe('none');
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
      new PointerEvent('pointerdown', {
        clientX: 10,
        clientY: 20,
        pointerType: 'touch',
        bubbles: true,
      }),
    );
    canvas.dispatchEvent(new WheelEvent('wheel', { deltaX: 0, deltaY: -120, bubbles: true }));
    globalThis.dispatchEvent(new KeyboardEvent('keydown', { key: 'a' }));
    canvas.dispatchEvent(new CompositionEvent('compositionend', { data: 'に' }));

    const kinds = t.sent.map((s) => s.msg.kind);
    expect(kinds).toContain('pointer');
    expect(kinds).toContain('wheel');
    expect(kinds).toContain('key');
    expect(kinds).toContain('composition');
    const pointer = t.sent.find((s) => s.msg.kind === 'pointer')!.msg as Extract<
      MainToWorker,
      { kind: 'pointer' }
    >;
    expect(pointer.action).toBe('down');
    expect(pointer.pointerKind).toBe('touch');
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

  it('wakes and drains Worker event deliveries through the RawHayate frame transaction', async () => {
    const canvas = mountCanvas();
    const t = fakeTransport();
    const handle = bootWorkerEngineBridge(canvas, {
      transport: t.transport,
      ime: recordingImeSink(),
      transferControlToOffscreen: () => ({}),
      dpr: 1,
    });
    const raw = createWorkerInputProxy(handle.shim);
    const wake = vi.fn();
    raw.set_request_redraw?.(wake);

    const registration = raw.register_listener(7, 0);
    const request = t.sent.find((entry) => entry.msg.kind === 'register-listener')?.msg as
      | Extract<MainToWorker, { kind: 'register-listener' }>
      | undefined;
    expect(request).toBeDefined();
    t.emit({
      kind: 'listener-registered',
      requestId: request!.requestId,
      listenerId: 73,
    });
    await expect(registration).resolves.toBe(73);

    const click = [73, 0, 7, 12, 18];
    t.emit({ kind: 'event-deliveries', rows: [click] });
    expect(wake).toHaveBeenCalledTimes(1);
    expect(raw.prepare_frame(16)).toEqual([1, click]);
    expect(raw.prepare_frame(32)).toEqual([2]);
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
    expect(t.sent.at(-1)?.msg).toEqual({ kind: 'detach' });
    expect(t.terminated).toBe(false);
    t.emit({ kind: 'detached' });
    expect(t.terminated).toBe(true);

    const before = t.sent.length;
    // detach 後の DOM 入力はもう Worker へ流れない（リスナ除去）。
    canvas.dispatchEvent(new PointerEvent('pointerdown', { clientX: 1, clientY: 1, bubbles: true }));
    handle.shim.pointer('down', 1, 1);
    expect(t.sent.length).toBe(before);
  });
});
