import { describe, expect, it, vi } from 'vitest';

// hayate-adapter-web* are external WASM packages (the system boundary) —
// web-host.test.ts always injects a fake `loadBackend` and never exercises
// the real routing here, so this is the one place that actually proves
// loadCanvasBackend (generated from wasm-build-manifest.json, #700/#703)
// imports the right bare specifier for each backend, and threads each
// backend's own runtime layer-present arg into init() (#717/#718).
const velloInit = vi.fn(async () => 'raw:vello');
vi.mock('@hayate/adapter-web', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: velloInit },
}));
const tinySkiaInit = vi.fn(async () => 'raw:tiny-skia');
vi.mock('@hayate/adapter-web-cpu', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: tinySkiaInit },
}));
const velloCpuInit = vi.fn(async () => 'raw:vello-cpu');
vi.mock('@hayate/adapter-web-vello-cpu', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: velloCpuInit },
}));

import { loadCanvasBackend } from './load-canvas-backend.generated.js';

const canvas = {} as HTMLCanvasElement;

describe('loadCanvasBackend (generated from wasm-build-manifest.json, #703)', () => {
  it('vello always loads @hayate/adapter-web, regardless of layerPresent (no separate layer-present package, #718)', async () => {
    await expect(loadCanvasBackend('vello', canvas)).resolves.toBe('raw:vello');
    await expect(loadCanvasBackend('vello', canvas, false)).resolves.toBe('raw:vello');
  });

  it('vello forwards layerPresent to init as the 2nd arg (ADR-0140 runtime flag, default ON)', async () => {
    await loadCanvasBackend('vello', canvas);
    expect(velloInit).toHaveBeenCalledWith(canvas, true);
  });

  it('vello forwards layerPresent=false when passed', async () => {
    await loadCanvasBackend('vello', canvas, false);
    expect(velloInit).toHaveBeenCalledWith(canvas, false);
  });

  it('tiny-skia loads @hayate/adapter-web-cpu (not @hayate/adapter-web-tiny-skia)', async () => {
    await expect(loadCanvasBackend('tiny-skia', canvas)).resolves.toBe('raw:tiny-skia');
  });

  it('vello-cpu loads @hayate/adapter-web-vello-cpu', async () => {
    await expect(loadCanvasBackend('vello-cpu', canvas)).resolves.toBe('raw:vello-cpu');
  });

  it('tiny-skia forwards cpuLayerPresent to init (ADR-0138 default ON)', async () => {
    await loadCanvasBackend('tiny-skia', canvas);
    expect(tinySkiaInit).toHaveBeenCalledWith(canvas, true);
  });

  it('tiny-skia forwards cpuLayerPresent=false when passed', async () => {
    await loadCanvasBackend('tiny-skia', canvas, true, false);
    expect(tinySkiaInit).toHaveBeenCalledWith(canvas, false);
  });

  it('vello-cpu forwards cpuLayerPresent to init (ADR-0138 default ON)', async () => {
    await loadCanvasBackend('vello-cpu', canvas);
    expect(velloCpuInit).toHaveBeenCalledWith(canvas, true);
  });
});
