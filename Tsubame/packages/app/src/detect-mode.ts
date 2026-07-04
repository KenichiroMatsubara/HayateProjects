export type Mode = 'DOM' | 'Canvas';
export type ModeSource = 'query' | 'auto';
export type RendererQuery = 'auto' | 'vello' | 'tiny-skia' | 'vello-cpu' | 'dom';
export type CanvasBackend = 'vello' | 'tiny-skia' | 'vello-cpu';

export interface DetectModeInput {
  rendererQuery: RendererQuery | null;
  hasEditContext: boolean;
  hasWebGPU: boolean;
}

export interface DetectModeResult {
  mode: Mode;
  backend?: CanvasBackend;
  source: ModeSource;
  renderer: RendererQuery;
}

/**
 * DOM / Canvas と Canvas バックエンドを決定する。
 *
 * - EditContext なし → DOM
 * - EditContext あり・WebGPU なし → Canvas (tiny-skia)
 * - EditContext あり・WebGPU あり → Canvas (vello)
 * - `?renderer=vello|tiny-skia|vello-cpu|dom` で明示指定
 */
export function detectMode(input: DetectModeInput): DetectModeResult {
  const { rendererQuery, hasEditContext, hasWebGPU } = input;

  if (rendererQuery === 'dom') {
    return { mode: 'DOM', source: 'query', renderer: 'dom' };
  }
  if (rendererQuery === 'vello') {
    return { mode: 'Canvas', backend: 'vello', source: 'query', renderer: 'vello' };
  }
  if (rendererQuery === 'tiny-skia') {
    return { mode: 'Canvas', backend: 'tiny-skia', source: 'query', renderer: 'tiny-skia' };
  }
  if (rendererQuery === 'vello-cpu') {
    return { mode: 'Canvas', backend: 'vello-cpu', source: 'query', renderer: 'vello-cpu' };
  }

  if (!hasEditContext) {
    return { mode: 'DOM', source: 'auto', renderer: 'auto' };
  }

  const backend: CanvasBackend = hasWebGPU ? 'vello' : 'tiny-skia';
  return { mode: 'Canvas', backend, source: 'auto', renderer: 'auto' };
}

export function parseRendererQuery(search: string): RendererQuery | null {
  const value = new URLSearchParams(search).get('renderer');
  if (
    value === 'auto' ||
    value === 'vello' ||
    value === 'tiny-skia' ||
    value === 'vello-cpu' ||
    value === 'dom'
  ) {
    return value;
  }
  return null;
}

export function detectModeFromSearch(search: string, env: {
  hasEditContext: boolean;
  hasWebGPU: boolean;
}): DetectModeResult {
  return detectMode({
    rendererQuery: parseRendererQuery(search),
    hasEditContext: env.hasEditContext,
    hasWebGPU: env.hasWebGPU,
  });
}
