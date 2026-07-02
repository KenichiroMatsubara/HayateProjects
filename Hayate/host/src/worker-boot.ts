/**
 * OffscreenCanvas＋単一 Worker への opt-in 配線（ADR-0128 web 半分の実 boot 側・#648）。
 *
 * `worker-host.ts` が定義する main↔Worker のメッセージ契約（{@link MainThreadShim} /
 * {@link WorkerEngineDispatcher}）を、実際の boot 経路から掴む「main スレッド側の橋渡し」を組む。
 * canvas を `transferControlToOffscreen()` で Worker へ transfer し、DOM の pointer/wheel/keyboard 入力を
 * shim 経由で Worker へ流し、Worker からの IME presentation を main の EditContext へ適用する。エンジン
 * 一式（WASM・layout・vello raster・Tsubame reactivity）は Worker 側で走り、main は「入力/IME を
 * postMessage で橋渡しする薄い shim」に徹する（診断 要因 2）。**既定は OFF・計測ゲート**（ADR-0128
 * 「native コミット・web は計測ゲート」）で、opt-in 時のみこの経路が起きる。
 */

import {
  MainThreadShim,
  type CanvasHandle,
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

/** {@link bootWorkerEngineBridge} の後始末。DOM 入力リスナを外し Worker を停止する（full reload で呼ぶ）。 */
export interface WorkerEngineBridgeHandle {
  readonly shim: MainThreadShim;
  readonly detach: () => void;
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
 * main スレッド側の Worker 橋渡しを組む（#648）。OffscreenCanvas を Worker へ transfer し、Worker の
 * エンジンを init する。DOM の pointer/wheel/keyboard 入力を shim 経由で Worker へ流し、Worker からの
 * IME presentation を main の EditContext へ適用する。返す `detach` はリスナ除去＋Worker 停止で、full
 * reload での安全な teardown / 再構築に使う。
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
  transport.onMessage((msg) => shim.handleWorkerMessage(msg));

  // canvas を Worker へ transfer してエンジンを init（COOP/COEP 不要）。以後 canvas の描画所有権は Worker。
  const offscreen = transferControlToOffscreen(canvas);
  shim.init(offscreen, canvas.width, canvas.height, dpr);

  // 入力を Worker へ橋渡しする main スレッドリスナ。座標は canvas ローカル（offsetX/offsetY）。
  const onPointerDown = (e: PointerEvent) => shim.pointer('down', e.offsetX, e.offsetY);
  const onPointerMove = (e: PointerEvent) => shim.pointer('move', e.offsetX, e.offsetY);
  const onPointerUp = (e: PointerEvent) => shim.pointer('up', e.offsetX, e.offsetY);
  const onWheel = (e: WheelEvent) => shim.wheel(e.offsetX, e.offsetY, e.deltaX, e.deltaY);
  const onKeyDown = (e: KeyboardEvent) => shim.key(e.key, keyModifiers(e));

  canvas.addEventListener('pointerdown', onPointerDown);
  canvas.addEventListener('pointermove', onPointerMove);
  canvas.addEventListener('pointerup', onPointerUp);
  canvas.addEventListener('wheel', onWheel);
  // keydown は EditContext 非フォーカス時も拾えるよう window で受ける（ADR-0069 の keydown 経路と同様）。
  // 非ブラウザ環境（globalThis に addEventListener が無い）では keydown 配線を省く（非 DOM 安全）。
  const keyTarget = globalThis as {
    addEventListener?: (t: string, cb: (e: KeyboardEvent) => void) => void;
    removeEventListener?: (t: string, cb: (e: KeyboardEvent) => void) => void;
  };
  keyTarget.addEventListener?.('keydown', onKeyDown);

  const detach = (): void => {
    canvas.removeEventListener('pointerdown', onPointerDown);
    canvas.removeEventListener('pointermove', onPointerMove);
    canvas.removeEventListener('pointerup', onPointerUp);
    canvas.removeEventListener('wheel', onWheel);
    keyTarget.removeEventListener?.('keydown', onKeyDown);
    transport.terminate();
  };

  return { shim, detach };
}

/**
 * Worker モードの main スレッド `RawHayate`（#648）。合成ルートが host-blind に受け取る `raw` の形を保つ
 * が、**エンジン一式は Worker 側で走る**（ADR-0128）。したがって input（pointer/key/text）は shim 経由で
 * Worker へ転送し、tree 構築 / apply_mutations / render など毎フレームのエンジン仕事は main では行わない
 * （no-op）。Worker が reactivity・layout・raster・a11y を単独所有するため、main の drive/query 面は不活性で
 * 正しい（穴埋めのスタブではなく、責務が Worker にある）。値を返す query は安全な既定を返す。
 */
export function createWorkerInputProxy(shim: MainThreadShim): RawHayate {
  const noop = (): void => undefined;
  return {
    // tree 構築・変異・描画は Worker が所有する（main では走らない）。
    element_create: noop,
    set_root: noop,
    element_append_child: noop,
    element_insert_before: noop,
    element_remove: noop,
    apply_mutations: noop,
    render: noop,
    set_background_color: noop,
    set_tuning: noop,
    register_listener: () => 0,
    // query 面は Worker 側 state を持たないので安全な既定（main は状態を持たない）。
    element_get_text: () => '',
    element_get_bounds: () => [0, 0, 0, 0],
    element_subtree_ids: () => new Float64Array(),
    has_selection: () => false,
    poll_accessibility: () => null,
    poll_events: () => [],
    element_effective_visual: () => null,
    // input は main が受けて Worker へ転送する（薄い shim の唯一の毎フレーム責務）。
    on_pointer_move: (x, y) => shim.pointer('move', x, y),
    on_pointer_down: (x, y) => shim.pointer('down', x, y),
    on_pointer_up: (x, y) => shim.pointer('up', x, y),
    on_wheel: (x, y, dx, dy) => shim.wheel(x, y, dx, dy),
    on_key_down: (key, modifiers) => shim.key(key, modifiers),
    on_text_input: (id, text) => shim.composition(id, text),
  };
}
