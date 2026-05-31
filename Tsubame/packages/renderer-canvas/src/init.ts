import initWasm, { HayateElementRenderer } from 'hayate-adapter-web';
import { CanvasRenderer } from './canvas-renderer.js';
import type { CanvasRendererOptions } from './canvas-renderer.js';

/**
 * Hayate WASM を初期化し CanvasRenderer を返す。
 * アプリ側はこの関数を呼ぶだけでよく、WASM ロードや HayateElementRenderer の
 * ライフサイクルを意識する必要がない。
 */
export async function initCanvasRenderer(
  canvas: HTMLCanvasElement,
  options?: CanvasRendererOptions,
): Promise<CanvasRenderer> {
  await initWasm();
  const hayate = await HayateElementRenderer.init(canvas);
  return new CanvasRenderer(hayate, options);
}
