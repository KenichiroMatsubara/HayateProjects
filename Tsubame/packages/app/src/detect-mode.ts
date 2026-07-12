export type Mode = 'DOM' | 'Canvas';
export type ModeSource = 'query' | 'auto' | 'default';
export type RendererQuery = 'auto' | 'skia-safe' | 'vello' | 'tiny-skia' | 'vello-cpu' | 'dom';
export type CanvasBackend = 'vello' | 'tiny-skia' | 'vello-cpu';

/**
 * クエリ未指定時の Web デモ既定レンダラ。skia-safe をプロジェクトの主力レンダラとして
 * 前面に出す（右上スイッチの既定選択もこれに揃える）。
 *
 * ただし skia-safe（rust-skia）は wasm32-unknown-unknown 非対応で **Web では動かせない**
 * （ADR-0146）。そこで Web ではラベルは skia-safe を保ちつつ、実 backend は skia 系 CPU
 * ラスタライザである tiny-skia へ委譲する（{@link SKIA_SAFE_WEB_BACKEND}）。ネイティブの
 * skia-safe Scene Renderer とは別実装だが、Web で選べる最も近い skia 系経路。
 */
export const DEFAULT_RENDERER: RendererQuery = 'skia-safe';

/** Web で `skia-safe` 選択時に実際にロードする WASM backend（ADR-0146 の Web 委譲先）。 */
export const SKIA_SAFE_WEB_BACKEND: CanvasBackend = 'tiny-skia';

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
 * - クエリ未指定 → 既定の skia-safe（Web は tiny-skia backend へ委譲・ADR-0146）
 * - `?renderer=skia-safe` → Canvas。Web では backend=tiny-skia（同上）
 * - `?renderer=vello|tiny-skia|vello-cpu|dom` で明示指定
 * - `?renderer=auto` → 環境プローブ:
 *   - EditContext なし → DOM
 *   - EditContext あり・WebGPU なし → Canvas (tiny-skia)
 *   - EditContext あり・WebGPU あり → Canvas (vello)
 */
export function detectMode(input: DetectModeInput): DetectModeResult {
  const { rendererQuery, hasEditContext, hasWebGPU } = input;

  if (rendererQuery === 'dom') {
    return { mode: 'DOM', source: 'query', renderer: 'dom' };
  }
  if (rendererQuery === 'skia-safe') {
    return { mode: 'Canvas', backend: SKIA_SAFE_WEB_BACKEND, source: 'query', renderer: 'skia-safe' };
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

  // クエリ未指定は既定レンダラ（skia-safe → Web は tiny-skia backend）。`auto` を明示した
  // 場合のみ従来の環境プローブに委ねる。
  if (rendererQuery === null) {
    return { mode: 'Canvas', backend: SKIA_SAFE_WEB_BACKEND, source: 'default', renderer: DEFAULT_RENDERER };
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
    value === 'skia-safe' ||
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
