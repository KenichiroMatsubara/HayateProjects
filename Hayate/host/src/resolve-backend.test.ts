import { describe, expect, it } from 'vitest';
import {
  parseRendererQueryBackend,
  resolveCanvasBackend,
  resolveCanvasBackendAttemptOrder,
  resolveCanvasBackendSelection,
  RENDERER_QUERY_PARAM,
  WEB_RENDERER_QUERY_VALUES,
  rendererOptimizationQueryParam,
  parseRendererOptimizationOptions,
} from './resolve-backend.js';

// Web の「タップで Scene Renderer を切り替える」操作面（Android の
// `adb shell am start -e hayate.renderer skia` と同じ操作感）の口。クエリ
// パラメータ `?renderer=vello|tiny-skia|vello-cpu` を強制指定として解釈する純ロジック。
// 値語彙は `SceneRendererKind::name()`（Rust）と同一。
describe('parseRendererQueryBackend', () => {
  it('parses a forced canvas backend from the renderer query param', () => {
    expect(parseRendererQueryBackend('?renderer=canvaskit')).toBe('canvaskit');
    expect(parseRendererQueryBackend('?renderer=vello')).toBe('vello');
    expect(parseRendererQueryBackend('?renderer=tiny-skia')).toBe('tiny-skia');
    expect(parseRendererQueryBackend('?renderer=vello-cpu')).toBe('vello-cpu');
  });

  it('defers (undefined) for auto, dom, unknown, or missing values', () => {
    // `auto` / `dom` は canvas backend の強制ではない（policy / mode 判定に委ねる）。
    expect(parseRendererQueryBackend('?renderer=auto')).toBeUndefined();
    expect(parseRendererQueryBackend('?renderer=dom')).toBeUndefined();
    expect(parseRendererQueryBackend('?renderer=canvas')).toBeUndefined();
    expect(parseRendererQueryBackend('')).toBeUndefined();
    expect(parseRendererQueryBackend(`?${RENDERER_QUERY_PARAM}=skia`)).toBeUndefined();
  });
});

// Renderer Selection Policy（Hayate CONTEXT）: WebGPU プローブ結果と任意の
// backend オーバーライドから、ロードすべき Scene Renderer の WASM バックエンドを
// 決める純ロジック。Render Host から分離した「if 文連鎖でない」ルール本体。
// レンダラ選択を「どれを / なぜ」の両方で観測可能にする（Android の
// `selected scene renderer:` ログ相当の web 版観測点）。優先順位は
// 明示 override > クエリ強制 > WebGPU 自動判定。
describe('resolveCanvasBackendSelection', () => {
  it('reports options-override when an explicit backend is given', () => {
    expect(resolveCanvasBackendSelection({ backend: 'tiny-skia' }, true, '?renderer=vello')).toEqual(
      { backend: 'tiny-skia', reason: 'options-override' },
    );
  });

  it('reports query-override when the renderer query forces a backend', () => {
    expect(resolveCanvasBackendSelection(undefined, true, '?renderer=canvaskit')).toEqual({
      backend: 'canvaskit',
      reason: 'query-override',
    });
    expect(resolveCanvasBackendSelection(undefined, true, '?renderer=tiny-skia')).toEqual({
      backend: 'tiny-skia',
      reason: 'query-override',
    });
    expect(resolveCanvasBackendSelection({}, false, '?renderer=vello')).toEqual({
      backend: 'vello',
      reason: 'query-override',
    });
  });

  it('selects CanvasKit as the first automatic boot candidate', () => {
    expect(resolveCanvasBackendSelection(undefined, true, '')).toEqual({
      backend: 'canvaskit',
      reason: 'canvaskit-auto',
    });
    // auto/dom クエリは強制ではないので自動判定に委ねる。
    expect(resolveCanvasBackendSelection(undefined, true, '?renderer=auto')).toEqual({
      backend: 'canvaskit',
      reason: 'canvaskit-auto',
    });
  });

  it('keeps CanvasKit first when WebGPU is unavailable', () => {
    expect(resolveCanvasBackendSelection(undefined, false, '')).toEqual({
      backend: 'canvaskit',
      reason: 'canvaskit-auto',
    });
  });
});

describe('WEB_RENDERER_QUERY_VALUES', () => {
  it('publishes the Host-owned renderer switch order', () => {
    expect(WEB_RENDERER_QUERY_VALUES).toEqual([
      'auto',
      'canvaskit',
      'vello',
      'tiny-skia',
      'vello-cpu',
    ]);
  });
});

describe('rendererOptimizationQueryParam', () => {
  it('keeps backend-specific optimization query mapping in the Host policy', () => {
    expect(rendererOptimizationQueryParam('vello')).toBe('layerPresent');
    expect(rendererOptimizationQueryParam('tiny-skia')).toBe('cpuLayerPresent');
    expect(rendererOptimizationQueryParam('vello-cpu')).toBe('cpuLayerPresent');
    expect(rendererOptimizationQueryParam('canvaskit')).toBe('layerPresent');
    expect(rendererOptimizationQueryParam('auto')).toBeUndefined();
    expect(rendererOptimizationQueryParam('dom')).toBeUndefined();
  });
});

describe('parseRendererOptimizationOptions', () => {
  it('parses optimization query flags inside the Host boundary', () => {
    expect(parseRendererOptimizationOptions('')).toEqual({
      layerPresent: true,
      cpuLayerPresent: true,
    });
    expect(parseRendererOptimizationOptions('?layerPresent=0&cpuLayerPresent=0')).toEqual({
      layerPresent: false,
      cpuLayerPresent: false,
    });
  });
});

describe('resolveCanvasBackendAttemptOrder', () => {
  it('uses CanvasKit → Vello → tiny-skia during automatic WebGPU boot', () => {
    expect(resolveCanvasBackendAttemptOrder(undefined, true)).toEqual([
      { backend: 'canvaskit', reason: 'canvaskit-auto' },
      { backend: 'vello', reason: 'webgpu-fallback' },
      { backend: 'tiny-skia', reason: 'webgpu-fallback' },
    ]);
  });

  it('skips Vello but retains CanvasKit → tiny-skia without WebGPU', () => {
    expect(resolveCanvasBackendAttemptOrder(undefined, false)).toEqual([
      { backend: 'canvaskit', reason: 'canvaskit-auto' },
      { backend: 'tiny-skia', reason: 'webgpu-unavailable-skip' },
    ]);
  });
});

describe('resolveCanvasBackend', () => {
  it('honours an explicit vello override regardless of WebGPU', () => {
    expect(resolveCanvasBackend({ backend: 'vello' }, false)).toBe('vello');
    expect(resolveCanvasBackend({ backend: 'vello' }, true)).toBe('vello');
  });

  it('honours an explicit tiny-skia override regardless of WebGPU', () => {
    expect(resolveCanvasBackend({ backend: 'tiny-skia' }, true)).toBe('tiny-skia');
    expect(resolveCanvasBackend({ backend: 'tiny-skia' }, false)).toBe('tiny-skia');
  });

  it('honours an explicit vello-cpu override regardless of WebGPU', () => {
    expect(resolveCanvasBackend({ backend: 'vello-cpu' }, true)).toBe('vello-cpu');
    expect(resolveCanvasBackend({ backend: 'vello-cpu' }, false)).toBe('vello-cpu');
  });

  it('auto-selects CanvasKit first when WebGPU is available and no override', () => {
    expect(resolveCanvasBackend(undefined, true)).toBe('canvaskit');
    expect(resolveCanvasBackend({}, true)).toBe('canvaskit');
  });

  it('auto-selects CanvasKit first when WebGPU is unavailable and no override', () => {
    expect(resolveCanvasBackend(undefined, false)).toBe('canvaskit');
    expect(resolveCanvasBackend({}, false)).toBe('canvaskit');
  });
});
