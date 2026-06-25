import { describe, expect, it } from 'vitest';
import { resolveCanvasBackend } from './resolve-backend.js';

// Renderer Selection Policy（Hayate CONTEXT）: WebGPU プローブ結果と任意の
// backend オーバーライドから、ロードすべき Scene Renderer の WASM バックエンドを
// 決める純ロジック。Render Host から分離した「if 文連鎖でない」ルール本体。
describe('resolveCanvasBackend', () => {
  it('honours an explicit vello override regardless of WebGPU', () => {
    expect(resolveCanvasBackend({ backend: 'vello' }, false)).toBe('vello');
    expect(resolveCanvasBackend({ backend: 'vello' }, true)).toBe('vello');
  });

  it('honours an explicit tiny-skia override regardless of WebGPU', () => {
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
