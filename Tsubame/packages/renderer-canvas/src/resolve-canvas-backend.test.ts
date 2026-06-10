import { describe, expect, it } from 'vitest';
import { resolveCanvasBackend } from './resolve-canvas-backend.js';

describe('resolveCanvasBackend', () => {
  it('forces vello when backend override is vello', () => {
    expect(resolveCanvasBackend({ backend: 'vello' }, false)).toBe('vello');
    expect(resolveCanvasBackend({ backend: 'vello' }, true)).toBe('vello');
  });

  it('forces tiny-skia when backend override is tiny-skia', () => {
    expect(resolveCanvasBackend({ backend: 'tiny-skia' }, true)).toBe('tiny-skia');
    expect(resolveCanvasBackend({ backend: 'tiny-skia' }, false)).toBe('tiny-skia');
  });

  it('auto-selects vello when WebGPU is available and no override', () => {
    expect(resolveCanvasBackend(undefined, true)).toBe('vello');
    expect(resolveCanvasBackend({}, true)).toBe('vello');
  });

  it('auto-selects tiny-skia when WebGPU is unavailable and no override', () => {
    expect(resolveCanvasBackend(undefined, false)).toBe('tiny-skia');
    expect(resolveCanvasBackend({}, false)).toBe('tiny-skia');
  });
});
