import { CanvasRenderer } from './canvas-renderer.js';
import type { CanvasRendererOptions } from './canvas-renderer.js';
import type { RawHayate } from './hayate.js';
import {
  resolveCanvasBackend,
  type CanvasBackend,
} from './resolve-canvas-backend.js';

export interface InitCanvasRendererOptions extends CanvasRendererOptions {
  /** WebGPU プローブ結果に関わらずロードする WASM バックエンド。 */
  backend?: CanvasBackend;
  /** 開発時専用の `tuning.json` テキスト。指定すると WASM レンダラに渡して
   * 味付け定数のデフォルトを上書きする。不正な JSON は無視され、ビルド時の
   * デフォルトが維持される。未指定なら上書きしない。 */
  tuning?: string;
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
  // を付けてはならない。構築時に DPR をキャッシュした重複 observer は、モバイル Chrome
  // （入力中のフォーカスズームで比率が変わる）でバッキングストアサイズを壊し、グリフを荒くする。
  // 入力・リサイズ・IME の所有権はアダプタにあり、Tsubame は resize / IME 経路に存在しない
  // （issue #475 / #474）。ホストはレンダーループの駆動だけを担う。
  return new CanvasRenderer(raw, { ...options, canvas });
}
