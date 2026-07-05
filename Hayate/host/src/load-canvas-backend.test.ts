import { describe, expect, it, vi } from 'vitest';

// hayate-adapter-web* are external WASM packages (the system boundary) —
// web-host.test.ts always injects a fake `loadBackend` and never exercises
// the real routing here, so this is the one place that actually proves
// loadCanvasBackend (generated from wasm-build-manifest.json, #700/#703)
// imports the right bare specifier for each (backend, layerPresent) pair.
vi.mock('hayate-adapter-web', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: vi.fn(async () => 'raw:vello') },
}));
vi.mock('hayate-adapter-web-layer-present', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: vi.fn(async () => 'raw:vello-layer-present') },
}));
vi.mock('hayate-adapter-web-cpu', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: vi.fn(async () => 'raw:tiny-skia') },
}));
vi.mock('hayate-adapter-web-vello-cpu', () => ({
  default: vi.fn(async () => {}),
  HayateElementRenderer: { init: vi.fn(async () => 'raw:vello-cpu') },
}));

import { loadCanvasBackend } from './load-canvas-backend.generated.js';

const canvas = {} as HTMLCanvasElement;

describe('loadCanvasBackend (generated from wasm-build-manifest.json, #703)', () => {
  it('vello with no layerPresent loads hayate-adapter-web-layer-present (ADR-0137 default ON)', async () => {
    await expect(loadCanvasBackend('vello', canvas)).resolves.toBe('raw:vello-layer-present');
  });

  it('vello with layerPresent=false loads hayate-adapter-web (ADR-0137 escape hatch)', async () => {
    await expect(loadCanvasBackend('vello', canvas, false)).resolves.toBe('raw:vello');
  });

  it('tiny-skia loads hayate-adapter-web-cpu (not hayate-adapter-web-tiny-skia)', async () => {
    await expect(loadCanvasBackend('tiny-skia', canvas)).resolves.toBe('raw:tiny-skia');
  });

  it('vello-cpu loads hayate-adapter-web-vello-cpu', async () => {
    await expect(loadCanvasBackend('vello-cpu', canvas)).resolves.toBe('raw:vello-cpu');
  });
});
