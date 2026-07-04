import manifest from '@hayate/protocol-spec/manifest' with { type: 'json' };
import { resolveCanvasBackend, type CanvasBackend } from './resolve-backend.js';
import {
  attachAccessibilityMirror,
  type AccessibilityMirror,
} from './accessibility-mirror.js';
import {
  bootWorkerEngineBridge,
  createWorkerInputProxy,
  shouldUseWorkerEngine,
  type WorkerTransport,
} from './worker-boot.js';
import type { CanvasHandle, MainEditContextSink } from './worker-host.js';
import type { RawHayate } from './raw-hayate.js';

export type { CanvasBackend } from './resolve-backend.js';
export type { RawHayate, HayateEffectiveVisual, HayateColorRecord } from './raw-hayate.js';
export { MainThreadShim, WorkerEngineDispatcher } from './worker-host.js';
export {
  bootWorkerEngineBridge,
  createWorkerInputProxy,
  shouldUseWorkerEngine,
  WORKER_ENGINE_QUERY_PARAM,
  WORKER_ENGINE_QUERY_VALUE,
  KEY_MODIFIER_SHIFT,
  KEY_MODIFIER_CTRL,
  KEY_MODIFIER_ALT,
  KEY_MODIFIER_META,
  type WorkerTransport,
  type WorkerEngineBridgeHandle,
  type BootWorkerEngineBridgeOptions,
} from './worker-boot.js';
export type {
  CanvasHandle,
  ImePresentation,
  MainEditContextSink,
  MainToWorker,
  WorkerEngine,
  WorkerToMain,
} from './worker-host.js';
export {
  attachAccessibilityMirror,
  ACCESSKIT_ROLE_TO_ARIA,
  A11Y_ROOT_ATTR,
  A11Y_NODE_ID_PREFIX,
  MIRROR_OPACITY,
  MIRROR_POINTER_EVENTS,
  type DetachAccessibilityMirror,
  type AccessibilityMirror,
} from './accessibility-mirror.js';

/**
 * このホストに焼き込まれた decoder の wire 定数バージョン。Miharashi はこれをバンドルの encoder
 * 版数と起動時に突き合わせ、一致時のみ mount する（#530 / CONTEXT「Protocol Version」）。decoder
 * （WASM）と同じ `@hayate/protocol-spec` の manifest version を source of truth とする。
 */
export const HOST_PROTOCOL_VERSION: number = manifest.version;

/**
 * web bootstrap が確立して合成ルートへ渡す host。`raw` は Hayate ランタイムのポート、
 * `requestFrame`/`cancelFrame` は host が確立した frame-clock。App はこれを
 * `new HayateRenderer({ raw, requestFrame, cancelFrame })` に渡して mount する。
 * browser / native は同じ形（`./native` の {@link import('./native.js').NativeHost}）。
 */
export interface WebHost {
  readonly raw: RawHayate;
  readonly requestFrame: (cb: FrameRequestCallback) => number;
  readonly cancelFrame: (handle: number) => void;
  /**
   * host のライフサイクル teardown。現状は Accessibility Mirror（ADR-0124）の root 除去を畳む。
   * ミラーは独立ループを持たず frame-clock に相乗りするため（#645）、レンダラ停止でミラーの tick も
   * 止まる。full reload 時に古い host を捨てる前に呼ぶ（`startMiharashiHost` が結線）。
   */
  readonly detach: () => void;
}

export interface CreateHayateWebHostOptions {
  /** WebGPU プローブ結果に関わらずロードする WASM バックエンド。 */
  backend?: CanvasBackend;
  /** 開発時専用の `tuning.json` テキスト。指定すると WASM レンダラに渡して味付け
   * 定数のデフォルトを上書きする。不正な JSON は無視され、ビルド時のデフォルトが
   * 維持される。未指定なら上書きしない。 */
  tuning?: string;
  /** clock 源。未指定ならブラウザの rAF。clock 源の確立は host bootstrap の責務。 */
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
  /** テスト注入 seam。既定は `navigator.gpu` プローブ。 */
  probeWebGPU?: () => Promise<boolean>;
  /** テスト注入 seam。既定は `hayate-adapter-web*` の動的 import + `init`。 */
  loadBackend?: (
    backend: CanvasBackend,
    canvas: HTMLCanvasElement,
  ) => Promise<RawHayate>;
  /**
   * テスト注入 seam。既定は `@hayate/host` の {@link attachAccessibilityMirror}（ADR-0124）。
   * canvas boot のたびに `(raw, canvas)` で呼ぶ。返った {@link AccessibilityMirror} の `poll` は
   * host が frame-clock に相乗りさせ（#645）、`detach` を `WebHost.detach` に通す。
   */
  attachMirror?: (
    raw: RawHayate,
    canvas: HTMLCanvasElement,
  ) => AccessibilityMirror;
  /**
   * OffscreenCanvas＋単一 Worker へエンジンを載せる opt-in（ADR-0128 web 半分・#648）。明示 true で
   * 有効、false で無効、未指定なら {@link locationSearch} のクエリパラメータで判定する。**既定は OFF**
   * （計測ゲート）。有効化には {@link spawnWorker} が要る（無ければ警告して従来の main 経路へフォールバック）。
   */
  workerEngine?: boolean;
  /** opt-in 判定に使う query 文字列。既定は `location.search`。テスト注入 seam。 */
  locationSearch?: string;
  /** Worker モードで main↔Worker transport を作る。実 `Worker` を包む。テスト注入 seam。 */
  spawnWorker?: () => WorkerTransport;
  /** Worker モードで Worker→main の IME presentation を適用する EditContext 面（ADR-0069）。既定 no-op。 */
  imeSink?: MainEditContextSink;
  /** `canvas.transferControlToOffscreen()` の注入 seam。既定は実 API。 */
  transferControlToOffscreen?: (canvas: HTMLCanvasElement) => CanvasHandle;
}

/** `navigator.gpu` で WebGPU の利用可否を判定する。 */
export async function probeWebGPU(): Promise<boolean> {
  try {
    const gpu = (navigator as { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
    if (!gpu) return false;
    const adapter = await gpu.requestAdapter();
    return adapter != null;
  } catch {
    return false;
  }
}

/**
 * 選択した backend の WASM を動的 import し、surface（canvas）上で `HayateElementRenderer`
 * を初期化して {@link RawHayate} を得る。canvas のコンテキスト型は一度決まると変えられ
 * ないため、WebGPU の可否を判定してから WASM 初期化に進む。
 */
async function loadCanvasBackend(
  backend: CanvasBackend,
  canvas: HTMLCanvasElement,
): Promise<RawHayate> {
  if (backend === 'vello') {
    const velloMod = await import('hayate-adapter-web');
    await velloMod.default();
    return (await velloMod.HayateElementRenderer.init(canvas)) as unknown as RawHayate;
  }
  if (backend === 'vello-cpu') {
    const velloCpuMod = await import('hayate-adapter-web-vello-cpu');
    await velloCpuMod.default();
    return (await velloCpuMod.HayateElementRenderer.init(canvas)) as unknown as RawHayate;
  }
  const cpuMod = await import('hayate-adapter-web-cpu');
  await cpuMod.default();
  return (await cpuMod.HayateElementRenderer.init(canvas)) as unknown as RawHayate;
}

/** no-op の EditContext 面。Worker モードで `imeSink` 未指定時の既定（IME 反映先が無い環境向け）。 */
const NOOP_IME_SINK: MainEditContextSink = {
  setKeyboardVisible: () => {},
  setCaretRect: () => {},
};

/**
 * Worker エンジン経路（#648）を組む。`spawnWorker` が無ければ（実 Worker を作れない）`null` を返し、
 * 呼び出し側は従来 main 経路へフォールバックする。成功時は canvas を Worker へ transfer し、入力/IME を
 * 橋渡しする shim を配線した {@link WebHost} を返す。`raw` は Worker がエンジンを所有する前提の input
 * proxy（drive/query は main では不活性）。`detach` は Worker 停止＋リスナ除去（full reload で再構築）。
 */
function tryCreateWorkerEngineHost(
  canvas: HTMLCanvasElement,
  options?: CreateHayateWebHostOptions,
): WebHost | null {
  const spawn = options?.spawnWorker;
  if (!spawn) return null;

  const transferControlToOffscreen =
    options?.transferControlToOffscreen ??
    ((c: HTMLCanvasElement) =>
      (c as unknown as { transferControlToOffscreen(): CanvasHandle }).transferControlToOffscreen());
  const dpr = typeof globalThis.devicePixelRatio === 'number' ? globalThis.devicePixelRatio : 1;

  const bridge = bootWorkerEngineBridge(canvas, {
    transport: spawn(),
    ime: options?.imeSink ?? NOOP_IME_SINK,
    transferControlToOffscreen,
    dpr,
  });

  const requestFrame =
    options?.requestFrame ?? ((cb: FrameRequestCallback) => globalThis.requestAnimationFrame(cb));
  const cancelFrame =
    options?.cancelFrame ?? ((handle: number) => globalThis.cancelAnimationFrame(handle));

  return {
    raw: createWorkerInputProxy(bridge.shim),
    requestFrame,
    cancelFrame,
    detach: bridge.detach,
  };
}

/**
 * Hayate の web Render Host を起動する：WebGPU をプローブし、Renderer Selection Policy
 * で backend を選び、WASM をロードして surface 上にレンダラを初期化し、{@link WebHost}
 * （`raw` + frame-clock）を返す。
 *
 * pointer / wheel 入力・resize 追従・IME は `hayate-adapter-web` が `HayateElementRenderer::init`
 * 内で自前配線・自己同期する（ADR-0080 / ADR-0069）。host は surface・clock を確立する
 * だけで、Tsubame の host-blind コアは raw + clock しか受け取らない（#476, #477）。
 */
export async function createHayateWebHost(
  canvas: HTMLCanvasElement,
  options?: CreateHayateWebHostOptions,
): Promise<WebHost> {
  // #648: OffscreenCanvas＋単一 Worker 経路の opt-in（既定 OFF・計測ゲート、ADR-0128）。有効時は main で
  // WASM をロードせず、canvas を Worker へ transfer してエンジンを Worker 側で走らせ、main は入力/IME を
  // 橋渡しする薄い shim に徹する（診断 要因 2）。無効時は以降の従来 main スレッド経路のまま挙動不変。
  const search =
    options?.locationSearch ??
    (typeof location !== 'undefined' ? location.search : undefined);
  if (shouldUseWorkerEngine(options?.workerEngine, search)) {
    const worker = tryCreateWorkerEngineHost(canvas, options);
    if (worker) return worker;
    // spawnWorker 未提供（実 Worker を作れない）等では従来 main 経路へフォールバックする。
    console.warn('createHayateWebHost: worker engine opt-in requested but unavailable; using main-thread path');
  }

  const probe = options?.probeWebGPU ?? probeWebGPU;
  const load = options?.loadBackend ?? loadCanvasBackend;
  const attachMirror = options?.attachMirror ?? attachAccessibilityMirror;

  const webgpuAvailable = await probe();
  const backend = resolveCanvasBackend(options, webgpuAvailable);
  const raw = await load(backend, canvas);

  // 開発時専用の味付け定数の上書き。最初のフレーム前に一度だけ適用する。不正な JSON は
  // WASM のセッタ内で throw するが、握りつぶしてコンパイル済みデフォルトへフォール
  // バックさせ、アプリを壊さない。
  if (options?.tuning != null) {
    try {
      raw.set_tuning(options.tuning);
    } catch (err) {
      console.warn('createHayateWebHost: ignoring invalid tuning.json', err);
    }
  }

  // 既定 clock はブラウザの rAF。lookup は tick 時まで遅延させ、非ブラウザ環境での
  // 構築（テスト等）が落ちないようにする。clock 源の確立は host bootstrap の責務。
  const baseRequestFrame =
    options?.requestFrame ?? ((cb: FrameRequestCallback) => globalThis.requestAnimationFrame(cb));
  const cancelFrame =
    options?.cancelFrame ?? ((handle: number) => globalThis.cancelAnimationFrame(handle));

  // Accessibility Mirror（ADR-0124）の attach 点。canvas+raw を握るこの 1 箇所で attach し、
  // teardown 用の detach を host に通す。本体は #592 が実装し、全 Canvas アプリへ自動で効く。
  const mirror = attachMirror(raw, canvas);

  // #645: ミラーはもう独立 rAF ループを持たない。host が返す frame-clock を包み、レンダラの各フレーム
  // コールバック末尾でミラーを 1 回 poll する（相乗り）。レンダラが idle に落ちて frame を出さなければ
  // ミラーの tick も走らず、frame-clock がアプリ全体で 1 本になる（診断 要因 1 / ADR-0126）。ミラー poll
  // は #642 の dirty ゲートで変更なしフレームはほぼ無償。フレーム後に poll するのでレイアウト後の bounds
  // を読める。
  const requestFrame = (cb: FrameRequestCallback): number =>
    baseRequestFrame((timestamp) => {
      cb(timestamp);
      mirror.poll();
    });

  return { raw, requestFrame, cancelFrame, detach: mirror.detach };
}
