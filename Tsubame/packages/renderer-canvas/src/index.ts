export { CanvasRenderer } from './canvas-renderer.js';
export type { CanvasRendererOptions } from './canvas-renderer.js';
export type { RawHayate } from './hayate.js';
export { parseColor } from './hayate.js';
export { initCanvasRenderer, probeWebGPU } from './init.js';
export type { InitCanvasRendererOptions } from './init.js';
// Android（埋め込み Hermes, ADR-0112）の API は WASM を巻き込まないよう
// 専用サブパス `@tsubame/renderer-canvas/android`（src/android.ts）に分離している。
export { resolveCanvasBackend } from './resolve-canvas-backend.js';
export type { CanvasBackend } from './resolve-canvas-backend.js';
export {
  encodeStylePatch,
  unsetKindsOf,
  TAG,
  UNSET_KIND,
} from '@tsubame/protocol-generated/codec';
export { OP, ELEMENT_KIND } from '@tsubame/protocol-generated/protocol';
