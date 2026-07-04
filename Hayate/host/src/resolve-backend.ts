export type CanvasBackend = 'vello' | 'tiny-skia' | 'vello-cpu';

export interface ResolveCanvasBackendOptions {
  backend?: CanvasBackend;
}

/**
 * WebGPU プローブ結果とオプションの backend オーバーライドから、ロードすべき
 * Canvas WASM バックエンド（Scene Renderer）を決める Renderer Selection Policy。
 * Render Host から分離し、host に埋め込んだ if 文連鎖にしない（Hayate CONTEXT）。
 */
export function resolveCanvasBackend(
  options: ResolveCanvasBackendOptions | undefined,
  webgpuAvailable: boolean,
): CanvasBackend {
  if (options?.backend !== undefined) return options.backend;
  return webgpuAvailable ? 'vello' : 'tiny-skia';
}
