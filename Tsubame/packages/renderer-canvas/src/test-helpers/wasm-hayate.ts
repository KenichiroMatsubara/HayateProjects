import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { RawHayate } from '../hayate.js';

export interface WasmHayateFixture {
  readonly raw: RawHayate;
  readonly canvas: HTMLCanvasElement;
  dispose(): void;
}

const wasmPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../../Hayate/wasm-pkgs/pkg-null/hayate_adapter_web_bg.wasm',
);

let wasmReady = false;

/** Load the null-backend WASM build for C3 integration tests (ADR-0055). */
export async function createNullHayate(
  width = 320,
  height = 240,
): Promise<WasmHayateFixture> {
  const mod = await import('hayate-adapter-web-null');
  if (!wasmReady) {
    mod.initSync({ module: readFileSync(wasmPath) });
    wasmReady = true;
  }

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  document.body.appendChild(canvas);

  const raw = (await mod.HayateElementRenderer.init(
    canvas,
  )) as unknown as RawHayate;

  // `init` derives the viewport from `canvas.getBoundingClientRect()`, but the
  // test DOM (jsdom/happy-dom) does no layout and reports a 0×0 box — leaving
  // the viewport at 0, so `width:100%/height:100%` collapses to zero geometry
  // and hit-testing/hover never fire. Set it explicitly from the known size.
  (raw as unknown as { set_viewport(w: number, h: number): void }).set_viewport(
    width,
    height,
  );

  return {
    raw,
    canvas,
    dispose() {
      (raw as { free?: () => void }).free?.();
      canvas.remove();
    },
  };
}
