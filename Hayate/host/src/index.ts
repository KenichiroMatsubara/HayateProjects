import manifest from '@hayate/protocol-spec/manifest' with { type: 'json' };
import { resolveCanvasBackend, type CanvasBackend } from './resolve-backend.js';
import {
  attachAccessibilityMirror,
  type DetachAccessibilityMirror,
} from './accessibility-mirror.js';
import type { RawHayate } from './raw-hayate.js';

export type { CanvasBackend } from './resolve-backend.js';
export type { RawHayate, HayateEffectiveVisual, HayateColorRecord } from './raw-hayate.js';
export {
  attachAccessibilityMirror,
  ACCESSKIT_ROLE_TO_ARIA,
  A11Y_ROOT_ATTR,
  MIRROR_OPACITY,
  MIRROR_POINTER_EVENTS,
  type DetachAccessibilityMirror,
  type AccessibilityMirrorOptions,
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
   * host のライフサイクル teardown。現状は Accessibility Mirror（ADR-0124）の root 除去と
   * rAF 停止を畳む。full reload 時に古い host を捨てる前に呼ぶ（`startMiharashiHost` が結線）。
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
   * canvas boot のたびに `(raw, canvas)` で呼び、返った detach を `WebHost.detach` に通す。
   */
  attachMirror?: (
    raw: RawHayate,
    canvas: HTMLCanvasElement,
  ) => DetachAccessibilityMirror;
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
  const cpuMod = await import('hayate-adapter-web-cpu');
  await cpuMod.default();
  return (await cpuMod.HayateElementRenderer.init(canvas)) as unknown as RawHayate;
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
  const requestFrame =
    options?.requestFrame ?? ((cb: FrameRequestCallback) => globalThis.requestAnimationFrame(cb));
  const cancelFrame =
    options?.cancelFrame ?? ((handle: number) => globalThis.cancelAnimationFrame(handle));

  // Accessibility Mirror（ADR-0124）の attach 点。canvas+raw を握るこの 1 箇所で attach し、
  // teardown 用の detach を host に通す。本体は #592 が実装し、全 Canvas アプリへ自動で効く。
  const detach = attachMirror(raw, canvas);

  return { raw, requestFrame, cancelFrame, detach };
}
