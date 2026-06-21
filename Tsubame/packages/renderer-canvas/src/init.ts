import { CanvasRenderer } from './canvas-renderer.js';
import type { CanvasRendererOptions } from './canvas-renderer.js';
import { attachTextInput } from './edit-context-sync.js';
import type { RawHayate } from './hayate.js';
import {
  resolveCanvasBackend,
  type CanvasBackend,
} from './resolve-canvas-backend.js';

export interface InitCanvasRendererOptions extends CanvasRendererOptions {
  /** WebGPU プローブ結果に関わらずロードする WASM バックエンド。 */
  backend?: CanvasBackend;
  /** Dev-only `tuning.json` text (#353 family). When provided it is handed to
   * the WASM renderer to overlay the taste-constant defaults; malformed JSON is
   * ignored so the compiled defaults stand. Absent → no override. */
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

  // Dev-only taste-constant override (#353 family): applied once before the
  // first frame. Malformed JSON throws inside the WASM setter; swallow it so a
  // bad `tuning.json` falls back to the compiled defaults rather than breaking
  // the app.
  if (options?.tuning != null) {
    try {
      raw.set_tuning(options.tuning);
    } catch (err) {
      console.warn('initCanvasRenderer: ignoring invalid tuning.json', err);
    }
  }

  // Pointer + wheel input *and* resize detection are self-wired by
  // hayate-adapter-web on `HayateElementRenderer::init` (ADR-0080). Its
  // ResizeObserver reads the live `devicePixelRatio` each fire, so the host must
  // not attach a second observer: a duplicate that cached its DPR at construction
  // would clobber the backing-store size on mobile Chrome (zoom-on-focus while
  // typing changes the ratio) and roughen glyphs. Resize ownership stays with the
  // adapter; the host only retains the EditContext IME / keyboard glue below.
  attachTextInput(canvas, raw);
  return new CanvasRenderer(raw, { ...options, canvas, autoResize: false });
}
