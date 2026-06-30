import { describe, expect, it, vi } from 'vitest';
import {
  MainThreadShim,
  WorkerEngineDispatcher,
  type ImePresentation,
  type MainEditContextSink,
  type MainToWorker,
  type WorkerEngine,
  type WorkerToMain,
} from './worker-host.js';

/**
 * 実 OffscreenCanvas/WASM/GPU/Worker を巻き込まずに、main↔Worker の橋渡し契約を検証する
 * （ADR-0128 Web 近似形）。transport（postMessage）は注入関数で直結し、main shim → Worker
 * dispatcher → main の往復を 1 スレッド上で観測する。
 */

/** エンジン呼び出しを記録する Worker 内エンジン。IME presentation は注入値を返す。 */
function recordingEngine(presentation: ImePresentation = { keyboardVisible: false, caretRect: null }) {
  const calls: string[] = [];
  const engine: WorkerEngine = {
    init: (_c, w, h, d) => calls.push(`init(${w},${h},${d})`),
    resize: (w, h, d) => calls.push(`resize(${w},${h},${d})`),
    onPointer: (a, x, y) => calls.push(`pointer(${a},${x},${y})`),
    onWheel: (x, y, dx, dy) => calls.push(`wheel(${x},${y},${dx},${dy})`),
    onKey: (k, m) => calls.push(`key(${k},${m})`),
    onComposition: (t, text) => calls.push(`composition(${t},${text})`),
    imePresentation: () => presentation,
  };
  return { engine, calls };
}

/** main shim → Worker dispatcher を直結した「チャネル」を組む（postMessage の代理）。 */
function wireBridge(opts?: { presentation?: ImePresentation }) {
  const { engine, calls } = recordingEngine(opts?.presentation);
  const imeSink: MainEditContextSink & {
    keyboardVisible: boolean;
    caretRect: ImePresentation['caretRect'];
  } = {
    keyboardVisible: false,
    caretRect: null,
    setKeyboardVisible(v) {
      this.keyboardVisible = v;
    },
    setCaretRect(r) {
      this.caretRect = r;
    },
  };
  const toMain: WorkerToMain[] = [];
  const toWorker: MainToWorker[] = [];

  // 前方参照を late-bind する（main→worker→main の循環）。
  let shim!: MainThreadShim;
  const dispatcher = new WorkerEngineDispatcher(engine, (msg) => {
    toMain.push(msg);
    shim.handleWorkerMessage(msg);
  });
  shim = new MainThreadShim((msg) => {
    toWorker.push(msg);
    dispatcher.handle(msg);
  }, imeSink);

  return { shim, dispatcher, engine, calls, imeSink, toMain, toWorker };
}

describe('OffscreenCanvas + Worker host bridge (ADR-0128 web)', () => {
  it('init transfers the canvas to the worker and the engine boots, replying ready', () => {
    const { shim, calls, toMain } = wireBridge();
    const canvas = { token: 'offscreen' };
    shim.init(canvas, 800, 600, 2);

    expect(calls).toContain('init(800,600,2)');
    expect(toMain).toContainEqual({ kind: 'ready' });
  });

  it('bridges DOM/pointer/wheel/key input from main to the worker engine', () => {
    const { shim, calls } = wireBridge();
    shim.pointer('down', 10, 20);
    shim.pointer('move', 11, 21);
    shim.wheel(5, 5, 0, -120);
    shim.key('a', 0);

    expect(calls).toEqual([
      'pointer(down,10,20)',
      'pointer(move,11,21)',
      'wheel(5,5,0,-120)',
      'key(a,0)',
    ]);
  });

  it('round-trips IME(EditContext) through the main<->worker bridge (ADR-0069)', () => {
    const presentation: ImePresentation = {
      keyboardVisible: true,
      caretRect: { x: 12, y: 34, width: 2, height: 18 },
    };
    const { shim, imeSink, toMain } = wireBridge({ presentation });

    // main で composition（EditContext 入力）→ Worker のエンジンへ → Worker が IME presentation を
    // 算出 → main の EditContext へ適用される。
    shim.composition(7, 'にほん');

    expect(toMain).toContainEqual({ kind: 'ime', presentation });
    expect(imeSink.keyboardVisible).toBe(true);
    expect(imeSink.caretRect).toEqual({ x: 12, y: 34, width: 2, height: 18 });
  });

  it('keeps the main shim engine-free so rendering never blocks the main/DOM thread', () => {
    // main shim はエンジン参照を持たず、入力は postMessage に変換するだけ（描画は Worker で走る）。
    const posted: MainToWorker[] = [];
    const imeSink: MainEditContextSink = {
      setKeyboardVisible: vi.fn(),
      setCaretRect: vi.fn(),
    };
    const shim = new MainThreadShim((m) => posted.push(m), imeSink);

    shim.pointer('down', 1, 2);
    shim.resize(640, 480, 1);

    // main 側は同期描画を一切走らせず、メッセージ列だけを生む。
    expect(posted).toEqual([
      { kind: 'pointer', action: 'down', x: 1, y: 2 },
      { kind: 'resize', width: 640, height: 480, dpr: 1 },
    ]);
  });

  it('uses structured-clone-safe messages (no SharedArrayBuffer / COOP-COEP free)', () => {
    // init を除く入力メッセージは plain data で、JSON 往復で同値（SharedArrayBuffer 非依存）。
    const captured: MainToWorker[] = [];
    const shim = new MainThreadShim((m) => captured.push(m), {
      setKeyboardVisible: () => {},
      setCaretRect: () => {},
    });
    shim.pointer('move', 3, 4);
    shim.key('Enter', 1);
    shim.composition(2, 'x');
    for (const msg of captured) {
      expect(JSON.parse(JSON.stringify(msg))).toEqual(msg);
    }
  });

  it('produces the same engine calls as a direct single-thread path (parity)', () => {
    // 同じ入力列を (a) エンジン直接 と (b) 橋渡し で流し、エンジン呼び出し列が一致する＝
    // スレッド分離（Worker 化）しても出力はシングルスレッド時と同値（DrawOp parity の前段）。
    const direct = recordingEngine();
    direct.engine.onPointer('down', 10, 20);
    direct.engine.onComposition(7, 'ほ');
    direct.engine.onKey('Enter', 0);

    const { shim, calls } = wireBridge();
    shim.pointer('down', 10, 20);
    shim.composition(7, 'ほ');
    shim.key('Enter', 0);

    expect(calls).toEqual(direct.calls);
  });
});
