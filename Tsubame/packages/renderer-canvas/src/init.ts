import { MODIFIER } from '@tsubame/protocol-generated/protocol';
import { CanvasRenderer } from './canvas-renderer.js';
import type { CanvasRendererOptions } from './canvas-renderer.js';
import {
  compositionFormatsToWire,
  type EditTextFormat,
} from './edit-context-sync.js';
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
  const webgpuAvailable = await probeWebGPU();
  const backend = resolveCanvasBackend(options, webgpuAvailable);
  const raw = await loadCanvasBackend(backend, canvas);

  // Pointer + wheel input is now self-wired by hayate-adapter-web on
  // `HayateElementRenderer::init` (ADR-0080). The host only retains the
  // EditContext IME / keyboard glue below.
  attachTextInput(canvas, raw);
  return new CanvasRenderer(raw, { ...options, canvas });
}

/** EditContext IME / keyboard path (adapterTier: deferred). */
function attachTextInput(canvas: HTMLCanvasElement, raw: RawHayate): void {
  if (typeof EditContext === 'undefined') return;

  canvas.tabIndex = 0;
  const editContext = new EditContext();
  canvas.editContext = editContext;
  let composing = false;
  // The composing segment's start offset (UTF-16) and current preedit text,
  // tracked so `textformatupdate` clause ranges can be made preedit-relative and
  // converted to UTF-8 byte offsets before crossing the wire (ADR-0102, #336).
  let composeBase = 0;
  let composeText = '';

  editContext.addEventListener('compositionstart', () => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = true;
    composeBase = editContext.selectionStart;
    composeText = '';
    raw.on_composition_start(id, '');
  });

  editContext.addEventListener('textupdate', (e: TextUpdateEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    const text = e.text ?? '';
    if (composing) {
      composeBase = e.updateRangeStart;
      composeText = text;
      // Plain (unformatted) update first; the conversion underlines arrive in
      // the `textformatupdate` that follows and re-sends with clause ranges.
      raw.on_composition_update(id, text);
    } else {
      raw.on_text_input(id, text);
    }
  });

  editContext.addEventListener('textformatupdate', (e: TextFormatUpdateEvent) => {
    if (!composing) return;
    const id = raw.focused_element_id();
    if (id === 0) return;
    const formats = e.getTextFormats() as unknown as EditTextFormat[];
    const wire = compositionFormatsToWire(composeText, composeBase, formats);
    raw.on_composition_update_formatted(id, composeText, wire);
  });

  editContext.addEventListener('compositionend', (e: CompositionEndEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = false;
    composeText = '';
    raw.on_composition_end(id, e.data ?? '');
  });

  canvas.addEventListener('keydown', (e) => {
    const id = raw.focused_element_id();
    // Selection keyboard gestures (Ctrl/Cmd+A, Shift+Arrow — #267) act on the
    // document-wide selection, so dispatch them even with nothing focused (a
    // read-only Selection Region). Core consumes selection keys internally.
    if (id === 0 && !raw.has_selection()) return;
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
