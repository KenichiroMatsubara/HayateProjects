import { describe, expect, it, vi } from 'vitest';

// hayate-adapter-web* are external WASM packages (the system boundary) —
// web-host.test.ts always injects a fake `loadBackend` and never exercises
// the real routing here, so this is the one place that actually proves
// loadCanvasBackend (generated from wasm-build-manifest.json, #700/#703)
// imports the right bare specifier for each backend and initializes the hard-cutover path.
const velloInit = vi.fn(async () => 'raw:vello');
vi.mock('@torimi/hayate-adapter-web', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: velloInit },
}));
const tinySkiaInit = vi.fn(async () => 'raw:tiny-skia');
vi.mock('@torimi/hayate-adapter-web-cpu', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: tinySkiaInit },
}));
import { loadCanvasBackend } from './load-canvas-backend.generated.js';

const canvas = {} as HTMLCanvasElement;

describe('loadCanvasBackend (generated from wasm-build-manifest.json, #703)', () => {
  it('vello loads @torimi/hayate-adapter-web', async () => {
    await expect(loadCanvasBackend('vello', canvas)).resolves.toBe('raw:vello');
    expect(velloInit).toHaveBeenCalledWith(canvas);
  });

  it('tiny-skia loads @torimi/hayate-adapter-web-cpu (not @torimi/hayate-adapter-web-tiny-skia)', async () => {
    await expect(loadCanvasBackend('tiny-skia', canvas)).resolves.toBe('raw:tiny-skia');
  });

  it('tiny-skia initializes without a legacy fallback flag', async () => {
    await loadCanvasBackend('tiny-skia', canvas);
    expect(tinySkiaInit).toHaveBeenCalledWith(canvas);
  });

});
