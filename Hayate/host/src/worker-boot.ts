/**
 * OffscreenCanvas＋単一 Worker への opt-in 配線（ADR-0128 web 半分の実 boot 側・#648）。
 *
 * `worker-host.ts` が定義する main↔Worker のメッセージ契約（{@link MainThreadShim} /
 * {@link WorkerEngineDispatcher}）を、実際の boot 経路から掴む「main スレッド側の橋渡し」を組む。
 * canvas を `transferControlToOffscreen()` で Worker へ transfer し、DOM の pointer/wheel/keyboard 入力を
 * shim 経由で Worker へ流し、Worker からの IME presentation を main の EditContext へ適用する。エンジン
 * 一式（WASM core・Render Host・selected Scene Renderer・common frame pipeline）は Worker 側で走り、
 * main は transport shim に徹する（診断 要因 2）。**既定は OFF・計測ゲート**（ADR-0128
 * 「native コミット・web は計測ゲート」）で、opt-in 時のみこの経路が起きる。
 */

import {
  MainThreadShim,
  type CanvasHandle,
  type FramePipelineObservation,
  type MainEditContextSink,
  type MainToWorker,
  type WorkerToMain,
} from './worker-host.js';
import type { RawHayate } from './raw-hayate.js';

/** opt-in を有効化するクエリパラメータ名と値（`?hayate-engine=worker`）。既定 OFF・計測ゲート。 */
export const WORKER_ENGINE_QUERY_PARAM = 'hayate-engine';
export const WORKER_ENGINE_QUERY_VALUE = 'worker';

/**
 * `KeyboardEvent` の修飾キーを shim の `key(key, modifiers)` へ渡す bitmask（名前付き。マジックナンバー
 * 回避）。Worker 側キーマップと同じ順序（shift/ctrl/alt/meta）で bit を割り当てる。
 */
export const KEY_MODIFIER_SHIFT = 1 << 0;
export const KEY_MODIFIER_CTRL = 1 << 1;
export const KEY_MODIFIER_ALT = 1 << 2;
export const KEY_MODIFIER_META = 1 << 3;
/** Graceful Shutdown barrier の応答が無い Worker を強制停止する上限。 */
export const WORKER_DETACH_TIMEOUT_MS = 1_000;
export const MIN_SURFACE_DIMENSION_PX = 1;
export const DEFAULT_DEVICE_PIXEL_RATIO = 1;

/**
 * main↔Worker の transport seam。実環境では `Worker`（`postMessage` / `onmessage` / `terminate`）を包み、
 * テストでは注入関数で直結する。OffscreenCanvas は `transfer` リストで渡す（SharedArrayBuffer 非依存＝
 * COOP/COEP 不要）。
 */
export interface WorkerTransport {
  postMessage(msg: MainToWorker, transfer?: Transferable[]): void;
  onMessage(cb: (msg: WorkerToMain) => void): void;
  terminate(): void;
}

export const WORKER_ENTRY_URL = new URL('./worker-entry.js', import.meta.url);
export const WORKER_NAME = 'hayate-offscreen-engine';

/** Browser の module Worker を一つだけ生成する production transport adapter。 */
export function createBrowserWorkerTransport(): WorkerTransport {
  if (typeof Worker === 'undefined') {
    throw new WorkerBootError('worker-unavailable', 'Web Worker is unavailable');
  }
  const worker = new Worker(WORKER_ENTRY_URL, {
    type: 'module',
    name: WORKER_NAME,
  });
  return {
    postMessage: (message, transfer) => worker.postMessage(message, transfer ?? []),
    onMessage: (callback) => {
      worker.onmessage = (event: MessageEvent<WorkerToMain>) => callback(event.data);
    },
    terminate: () => worker.terminate(),
  };
}

/** {@link bootWorkerEngineBridge} の後始末。DOM 入力リスナを外し Worker を停止する（full reload で呼ぶ）。 */
export interface WorkerEngineBridgeHandle {
  readonly shim: MainThreadShim;
  /** Worker 内の WASM core / Render Host / Scene Renderer が使用可能になるまで待つ。 */
  readonly ready: Promise<void>;
  readonly pipelineObservation: () => Promise<FramePipelineObservation>;
  readonly detach: () => void;
}

export type WorkerBootFailureCode =
  | 'worker-unavailable'
  | 'offscreen-canvas-unavailable'
  | 'renderer-init-failed';

/** Worker boot が成立しなかったことを main 側の呼び出し元へ保ったまま返す typed failure。 */
export class WorkerBootError extends Error {
  override readonly name = 'WorkerBootError';

  constructor(
    readonly code: WorkerBootFailureCode,
    message: string,
  ) {
    super(message);
  }
}

export interface BootWorkerEngineBridgeOptions {
  /** main↔Worker の transport（既定は実 `Worker` を包んだアダプタ）。 */
  readonly transport: WorkerTransport;
  /** main の EditContext 面（ADR-0069）。Worker からの IME presentation を適用する。 */
  readonly ime: MainEditContextSink;
  /** `canvas.transferControlToOffscreen()` の注入 seam。テストではトークンを返す。 */
  readonly transferControlToOffscreen: (canvas: HTMLCanvasElement) => CanvasHandle;
  /** device pixel ratio。init で Worker のサーフェス metrics に渡す。 */
  readonly dpr: number;
}

export function workerSurfaceMetrics(
  cssWidth: number,
  cssHeight: number,
  dpr: number,
): { width: number; height: number; dpr: number } {
  const scale = Number.isFinite(dpr) && dpr > 0 ? dpr : DEFAULT_DEVICE_PIXEL_RATIO;
  return {
    width: Math.max(MIN_SURFACE_DIMENSION_PX, Math.round(Math.max(0, cssWidth) * scale)),
    height: Math.max(MIN_SURFACE_DIMENSION_PX, Math.round(Math.max(0, cssHeight) * scale)),
    dpr: scale,
  };
}

/**
 * opt-in（明示フラグ or クエリパラメータ）で Worker エンジン経路を使うか判定する。明示フラグが与えられ
 * ればそれを優先し、無ければ `location.search` の {@link WORKER_ENGINE_QUERY_PARAM} を見る。既定 OFF。
 */
export function shouldUseWorkerEngine(
  explicit: boolean | undefined,
  search: string | undefined,
): boolean {
  if (explicit != null) return explicit;
  if (!search) return false;
  return new URLSearchParams(search).get(WORKER_ENGINE_QUERY_PARAM) === WORKER_ENGINE_QUERY_VALUE;
}

/** `KeyboardEvent` から shim へ渡す修飾 bitmask を組む。 */
function keyModifiers(e: KeyboardEvent): number {
  return (
    (e.shiftKey ? KEY_MODIFIER_SHIFT : 0) |
    (e.ctrlKey ? KEY_MODIFIER_CTRL : 0) |
    (e.altKey ? KEY_MODIFIER_ALT : 0) |
    (e.metaKey ? KEY_MODIFIER_META : 0)
  );
}

/**
 * main スレッド側の Worker 橋渡しを組む（#903）。OffscreenCanvas を Worker へ transfer し、Worker の
 * エンジンを init する。DOM の pointer/wheel/keyboard 入力を shim 経由で Worker へ流し、Worker からの
 * IME presentation を main の EditContext へ適用する。返す `detach` はリスナ除去＋Worker 停止で、full
 * reload での安全な teardown / 再構築に使う。終了時は Worker 内の common pipeline が Shutdown barrier
 * を完了してから `terminate()` する。
 */
export function bootWorkerEngineBridge(
  canvas: HTMLCanvasElement,
  options: BootWorkerEngineBridgeOptions,
): WorkerEngineBridgeHandle {
  const { transport, ime, transferControlToOffscreen, dpr } = options;

  const shim = new MainThreadShim(
    (msg, transfer) => transport.postMessage(msg, transfer),
    ime,
  );
  let resolveReady!: () => void;
  let rejectReady!: (error: WorkerBootError) => void;
  const ready = new Promise<void>((resolve, reject) => {
    resolveReady = resolve;
    rejectReady = reject;
  });
  let detached = false;
  let detachTimer: ReturnType<typeof setTimeout> | undefined;
  const terminate = (): void => {
    if (detached) return;
    detached = true;
    if (detachTimer != null) clearTimeout(detachTimer);
    transport.terminate();
  };
  transport.onMessage((msg) => {
    if (msg.kind === 'ready') {
      resolveReady();
    } else if (msg.kind === 'boot-failure') {
      rejectReady(new WorkerBootError(msg.failure.code, msg.failure.message));
    } else if (msg.kind === 'detached') {
      terminate();
    }
    shim.handleWorkerMessage(msg);
  });

  // canvas を Worker へ transfer してエンジンを init（COOP/COEP 不要）。以後 canvas の描画所有権は Worker。
  const cssRect = canvas.getBoundingClientRect();
  const initialMetrics =
    cssRect.width > 0 && cssRect.height > 0
      ? workerSurfaceMetrics(cssRect.width, cssRect.height, dpr)
      : { width: canvas.width, height: canvas.height, dpr };
  const offscreen = transferControlToOffscreen(canvas);
  shim.init(offscreen, initialMetrics.width, initialMetrics.height, initialMetrics.dpr);

  // 入力を Worker へ橋渡しする main スレッドリスナ。座標は canvas ローカル（offsetX/offsetY）。
  const pointerKind = (event: PointerEvent): 'mouse' | 'touch' | 'pen' =>
    event.pointerType === 'touch' || event.pointerType === 'pen'
      ? event.pointerType
      : 'mouse';
  const onPointerDown = (e: PointerEvent) =>
    shim.pointer('down', e.offsetX, e.offsetY, pointerKind(e));
  const onPointerMove = (e: PointerEvent) =>
    shim.pointer('move', e.offsetX, e.offsetY, pointerKind(e));
  const onPointerUp = (e: PointerEvent) =>
    shim.pointer('up', e.offsetX, e.offsetY, pointerKind(e));
  const onWheel = (e: WheelEvent) => shim.wheel(e.offsetX, e.offsetY, e.deltaX, e.deltaY);
  const onKeyDown = (e: KeyboardEvent) => shim.key(e.key, keyModifiers(e));
  const onCompositionEnd = (e: CompositionEvent) => shim.composition(0, e.data);

  canvas.addEventListener('pointerdown', onPointerDown);
  canvas.addEventListener('pointermove', onPointerMove);
  canvas.addEventListener('pointerup', onPointerUp);
  canvas.addEventListener('wheel', onWheel);
  canvas.addEventListener('compositionend', onCompositionEnd);
  // keydown は EditContext 非フォーカス時も拾えるよう window で受ける（ADR-0069 の keydown 経路と同様）。
  // 非ブラウザ環境（globalThis に addEventListener が無い）では keydown 配線を省く（非 DOM 安全）。
  const keyTarget = globalThis as {
    addEventListener?: (t: string, cb: (e: KeyboardEvent) => void) => void;
    removeEventListener?: (t: string, cb: (e: KeyboardEvent) => void) => void;
  };
  keyTarget.addEventListener?.('keydown', onKeyDown);

  const resizeObserver =
    typeof ResizeObserver === 'undefined'
      ? undefined
      : new ResizeObserver((entries) => {
          const rect = entries[0]?.contentRect;
          if (!rect) return;
          const metrics = workerSurfaceMetrics(rect.width, rect.height, dpr);
          shim.resize(metrics.width, metrics.height, metrics.dpr);
        });
  resizeObserver?.observe(canvas);

  let detachRequested = false;
  const detach = (): void => {
    if (detachRequested) return;
    detachRequested = true;
    canvas.removeEventListener('pointerdown', onPointerDown);
    canvas.removeEventListener('pointermove', onPointerMove);
    canvas.removeEventListener('pointerup', onPointerUp);
    canvas.removeEventListener('wheel', onWheel);
    canvas.removeEventListener('compositionend', onCompositionEnd);
    keyTarget.removeEventListener?.('keydown', onKeyDown);
    resizeObserver?.disconnect();
    ime.setKeyboardVisible(false);
    ime.setCaretRect(null);
    shim.detach();
    detachTimer = setTimeout(terminate, WORKER_DETACH_TIMEOUT_MS);
  };

  return {
    shim,
    ready,
    pipelineObservation: () => shim.pipelineObservation(),
    detach,
  };
}

/**
 * Worker モードの main スレッド `RawHayate`（#648）。合成ルートが host-blind に受け取る `raw` の形を保つ
 * が、**エンジン一式は Worker 側で走る**（ADR-0128）。mutation / frame / input は値として転送し、
 * main では core commit・layout・pipeline admission・Scene Renderer present を実行しない。同期 query は
 * Worker state を main に複製しないため安全な既定を返す。
 */
export function createWorkerInputProxy(shim: MainThreadShim): RawHayate {
  const noop = (): void => undefined;
  let frameId = 0;
  const prepared = new Map<number, number>();
  return {
    // main は mutation 値を解釈せず、Worker 内の WASM core へ transport するだけ。
    element_create: (id, elementKind) =>
      shim.mutation({ kind: 'element-create', id, elementKind }),
    set_root: (id) => shim.mutation({ kind: 'set-root', id }),
    element_append_child: (parent, child) =>
      shim.mutation({ kind: 'append-child', parent, child }),
    element_insert_before: (parent, child, before) =>
      shim.mutation({ kind: 'insert-before', parent, child, before }),
    element_remove: (id) => shim.mutation({ kind: 'remove', id }),
    apply_mutations: (ops, styles, texts, draws) =>
      shim.mutation({
        kind: 'apply-mutations',
        ops: ops.slice(),
        styles: styles.slice(),
        texts: [...texts],
        draws: draws.slice(),
      }),
    render: (timestampMs) => shim.frame(timestampMs),
    prepare_frame: (timestampMs) => {
      const id = ++frameId;
      prepared.set(id, timestampMs);
      return [id];
    },
    commit_frame: (id) => {
      const timestampMs = prepared.get(id);
      if (timestampMs == null) return;
      prepared.delete(id);
      shim.frame(timestampMs);
    },
    abort_frame: (id) => {
      prepared.delete(id);
    },
    set_background_color: (r, g, b) =>
      shim.mutation({ kind: 'background', r, g, b }),
    set_tuning: noop,
    register_listener: () => 0,
    // query 面は Worker 側 state を持たないので安全な既定（main は状態を持たない）。
    element_get_text: () => '',
    element_get_bounds: () => [0, 0, 0, 0],
    element_subtree_ids: () => new Float64Array(),
    has_selection: () => false,
    // 描画は Worker が所有するので、main 側 proxy に pending visual work は存在しない。
    has_pending_visual_work: () => false,
    poll_accessibility: () => null,
    poll_events: () => [],
    element_effective_visual: () => null,
    // input は main が受けて Worker へ転送する（薄い shim の唯一の毎フレーム責務）。
    on_pointer_move: (x, y) => shim.pointer('move', x, y),
    on_pointer_down: (x, y) => shim.pointer('down', x, y),
    on_pointer_up: (x, y) => shim.pointer('up', x, y),
    on_wheel: (x, y, dx, dy) => shim.wheel(x, y, dx, dy),
    on_key_down: (key, modifiers) => shim.key(key, modifiers),
    dispatch_edit_intent: (target, intent) => {
      shim.editIntent(target, intent);
      return 2;
    },
    on_text_input: (id, text) => shim.composition(id, text),
  };
}
