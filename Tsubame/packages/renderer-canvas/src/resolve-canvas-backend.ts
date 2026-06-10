export type CanvasBackend = 'vello' | 'tiny-skia';

export interface ResolveCanvasBackendOptions {
  backend?: CanvasBackend;
}

/**
 * WebGPU プローブ結果とオプションの backend オーバーライドから
 * ロードすべき Canvas WASM バックエンドを決定する。
 */
export function resolveCanvasBackend(
  options: ResolveCanvasBackendOptions | undefined,
  webgpuAvailable: boolean,
): CanvasBackend {
  if (options?.backend !== undefined) return options.backend;
  return webgpuAvailable ? 'vello' : 'tiny-skia';
}
