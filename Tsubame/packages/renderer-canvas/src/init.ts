import { MODIFIER } from '@tsubame/protocol-generated/protocol';
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
  // Sync the canvas pixel buffer to its current CSS layout size before WASM init.
  // CSS (position:fixed; inset:0; width:100vw; height:100vh) drives the display
  // size; we read it here so app code never needs to know about canvas dimensions.
  const rect = canvas.getBoundingClientRect();
  canvas.width = Math.round(rect.width);
  canvas.height = Math.round(rect.height);

  const webgpuAvailable = await probeWebGPU();
  const backend = resolveCanvasBackend(options, webgpuAvailable);
  const raw = await loadCanvasBackend(backend, canvas);

  attachPointerInput(canvas, raw);
  attachTextInput(canvas, raw);
  return new CanvasRenderer(raw, { ...options, canvas });
}

function attachPointerInput(canvas: HTMLCanvasElement, raw: RawHayate): void {
  const toCanvas = (e: MouseEvent): readonly [number, number] => {
    const rect = canvas.getBoundingClientRect();
    const sx = rect.width === 0 ? 1 : canvas.width / rect.width;
    const sy = rect.height === 0 ? 1 : canvas.height / rect.height;
    return [(e.clientX - rect.left) * sx, (e.clientY - rect.top) * sy];
  };
  canvas.addEventListener('mousemove', (e) => {
    const [x, y] = toCanvas(e);
    raw.on_pointer_move(x, y);
  });
  canvas.addEventListener('mousedown', (e) => {
    const [x, y] = toCanvas(e);
    raw.on_pointer_down(x, y);
  });
  canvas.addEventListener('mouseup', (e) => {
    const [x, y] = toCanvas(e);
    raw.on_pointer_up(x, y);
  });
  canvas.addEventListener(
    'wheel',
    (e) => {
      const [x, y] = toCanvas(e);
      raw.on_wheel(x, y, e.deltaX, e.deltaY);
    },
    { passive: true },
  );
}

/** EditContext IME / keyboard path (adapterTier: deferred). */
function attachTextInput(canvas: HTMLCanvasElement, raw: RawHayate): void {
  if (typeof EditContext === 'undefined') return;

  canvas.tabIndex = 0;
  const editContext = new EditContext();
  canvas.editContext = editContext;
  let composing = false;

  editContext.addEventListener('compositionstart', () => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = true;
    raw.on_composition_start(id, '');
  });

  editContext.addEventListener('textupdate', (e: TextUpdateEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    const text = e.text ?? '';
    if (composing) {
      raw.on_composition_update(id, text);
    } else {
      raw.on_text_input(id, text);
    }
  });

  editContext.addEventListener('compositionend', (e: CompositionEndEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = false;
    raw.on_composition_end(id, e.data ?? '');
  });

  canvas.addEventListener('keydown', (e) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    if (composing && e.key !== 'Escape') {
      e.preventDefault();
      return;
    }

    let mods = 0;
    if (e.shiftKey) mods |= MODIFIER.SHIFT;
    if (e.ctrlKey) mods |= MODIFIER.CTRL;
    if (e.altKey) mods |= MODIFIER.ALT;
    if (e.metaKey) mods |= MODIFIER.META;
    raw.on_key_down(e.key, mods);

    const isPrintable = e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey;
    if (!isPrintable) {
      e.preventDefault();
    }
  });
}
