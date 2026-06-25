import { CanvasRenderer } from './canvas-renderer.js';
import type { RawHayate } from './hayate.js';
import {
  resolveCanvasBackend,
  type CanvasBackend,
} from './resolve-canvas-backend.js';

export interface InitCanvasRendererOptions {
  /** WebGPU プローブ結果に関わらずロードする WASM バックエンド。 */
  backend?: CanvasBackend;
  /** 開発時専用の `tuning.json` テキスト。指定すると WASM レンダラに渡して
   * 味付け定数のデフォルトを上書きする。不正な JSON は無視され、ビルド時の
   * デフォルトが維持される。未指定なら上書きしない。 */
  tuning?: string;
  /** host が確立する frame-clock（ADR-0004）。未指定ならブラウザの rAF を使う。
   * clock 源の確立は host bootstrap の責務で、host-blind コアは既定を持たない。 */
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
}

export async function probeWebGPU(): Promise<boolean> {
  try {
    const gpu = (navigator as any).gpu;
    if (!gpu) return false;
    const adapter = await gpu.requestAdapter();
    return adapter != null;
  } catch {
    return false;
  }
}

/**
 * Hayate WASM を初期化し `CanvasRenderer` を返す。
 *
 * WebGPU が利用可能なら Vello バックエンドを、利用不可なら
 * tiny-skia CPU バックエンドをロードする。
 *
 * canvas のコンテキスト型は一度決まると変更できないため、
 * WebGPU の可否を事前に判定してから WASM 初期化に進む。
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

export async function initCanvasRenderer(
  canvas: HTMLCanvasElement,
  options?: InitCanvasRendererOptions,
): Promise<CanvasRenderer> {
  const webgpuAvailable = await probeWebGPU();
  const backend = resolveCanvasBackend(options, webgpuAvailable);
  const raw = await loadCanvasBackend(backend, canvas);

  // 開発時専用の味付け定数の上書き。最初のフレーム前に一度だけ適用する。
  // 不正な JSON は WASM のセッタ内で throw するが、握りつぶしてビルド時の
  // デフォルトにフォールバックさせ、アプリを壊さない。
  if (options?.tuning != null) {
    try {
      raw.set_tuning(options.tuning);
    } catch (err) {
      console.warn('initCanvasRenderer: ignoring invalid tuning.json', err);
    }
  }

  // ポインタ + ホイール入力・リサイズ検知・IME（EditContext / keydown）は
  // すべて hayate-adapter-web が `HayateElementRenderer::init` で自前で結線する
  // （ADR-0080 / ADR-0069）。その ResizeObserver は発火ごとに最新の `devicePixelRatio`
  // を読み、`tree.set_viewport` を WASM 内で直接駆動するため、ホストは 2 つ目の observer
  // を付けてはならない。入力・リサイズ・IME の所有権はアダプタにあり、Tsubame は
  // resize / IME 経路に存在しない（issue #475 / #474）。host はここで clock を確立し、
  // host-blind コアに raw + clock を渡してレンダーループを駆動するだけ（#476, ADR-0004）。
  const renderer = new CanvasRenderer({
    raw,
    requestFrame:
      options?.requestFrame ?? globalThis.requestAnimationFrame.bind(globalThis),
    cancelFrame:
      options?.cancelFrame ?? globalThis.cancelAnimationFrame.bind(globalThis),
  });
  renderer.start();
  return renderer;
}
