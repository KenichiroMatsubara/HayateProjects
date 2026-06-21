// Android（埋め込み Hermes, ADR-0112）専用の公開エントリ。
//
// パッケージ index（`./index.ts`）は `init.ts` 経由でブラウザ用 WASM
// （`hayate-adapter-web*`）を動的 import するため、IIFE バンドルに巨大な WASM
// base64 を巻き込む。Android はネイティブ cdylib を使い WASM 不要なので、
// WASM へ到達しない経路だけをこのエントリから re-export する。
export { CanvasRenderer } from './canvas-renderer.js';
export type { CanvasRendererOptions } from './canvas-renderer.js';
export type { RawHayate } from './hayate.js';
export { parseColor } from './hayate.js';
export { createAndroidCanvasRenderer } from './init-android.js';
export type {
  AndroidCanvasRendererHandle,
  AndroidCanvasRendererOptions,
} from './init-android.js';
