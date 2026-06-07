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
  '../../../../../Hayate/examples/web-demo/pkg-null/hayate_adapter_web_bg.wasm',
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

  return {
    raw,
    canvas,
    dispose() {
      (raw as { free?: () => void }).free?.();
      canvas.remove();
    },
  };
}
