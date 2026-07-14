// Browser UI が backend 名を複製せずに選択肢を構築するための軽量な public entry。
// Host bootstrap や CanvasKit loader は import しない。
export {
  RENDERER_QUERY_PARAM,
  RENDERER_VALUE_CANVASKIT,
  RENDERER_VALUE_VELLO,
  RENDERER_VALUE_TINY_SKIA,
  RENDERER_VALUE_VELLO_CPU,
  WEB_RENDERER_QUERY_VALUES,
  rendererOptimizationQueryParam,
  parseRendererOptimizationOptions,
} from './resolve-backend.js';
export type {
  WebRendererOptimizationQueryParam,
  RendererOptimizationOptions,
} from './resolve-backend.js';
