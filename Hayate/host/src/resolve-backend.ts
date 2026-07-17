export type CanvasBackend = 'vello' | 'tiny-skia';

export interface ResolveCanvasBackendOptions {
  backend?: CanvasBackend;
}

/**
 * Scene Renderer を強制指定するクエリパラメータのキー。Android の `hayate.renderer`
 * intent extra（`adb shell am start -e hayate.renderer skia`）の web 対応物で、WASM を
 * 作り直さずタップ／ディープリンクでレンダラを切り替える口（ADR-0050 の web 側）。
 */
export const RENDERER_QUERY_PARAM = 'renderer';

/**
 * 強制指定の値語彙。`SceneRendererKind::name()`（Rust）および Android の
 * `RENDERER_VALUE_*` と同一の安定 ID を使う。
 */
export const RENDERER_VALUE_VELLO = 'vello';
export const RENDERER_VALUE_TINY_SKIA = 'tiny-skia';

/** Web Host が公開する backend 選択 UI の安定語彙（表示順も policy の一部）。 */
export const WEB_RENDERER_QUERY_VALUES = [
  'auto',
  RENDERER_VALUE_VELLO,
  RENDERER_VALUE_TINY_SKIA,
] as const;

export type WebRendererOptimizationQueryParam = 'layerPresent' | 'cpuLayerPresent';

/** 選択中 backend に対応する比較用 optimization query。UI は対応表を複製しない。 */
export function rendererOptimizationQueryParam(
  renderer: string,
): WebRendererOptimizationQueryParam | undefined {
  switch (renderer) {
    case RENDERER_VALUE_VELLO:
      return 'layerPresent';
    case RENDERER_VALUE_TINY_SKIA:
      return 'cpuLayerPresent';
    default:
      return undefined;
  }
}

export interface RendererOptimizationOptions {
  layerPresent: boolean;
  cpuLayerPresent: boolean;
}

/** URL の比較用 optimization flags も Host 内で解釈する。両経路とも既定は ON。 */
export function parseRendererOptimizationOptions(search: string): RendererOptimizationOptions {
  const params = new URLSearchParams(search);
  return {
    layerPresent: params.get('layerPresent') !== '0',
    cpuLayerPresent: params.get('cpuLayerPresent') !== '0',
  };
}

/**
 * `?renderer=vello|tiny-skia` を強制指定として解釈する。
 * `auto` / `dom` / 未知値 / 未指定は canvas backend の強制ではないので `undefined`
 * （＝自動選択に委ねる）。`dom` は Web entry が Hayate Host を起動しないための退避値。
 */
export function parseRendererQueryBackend(search: string): CanvasBackend | undefined {
  const value = new URLSearchParams(search).get(RENDERER_QUERY_PARAM);
  switch (value) {
    case RENDERER_VALUE_VELLO:
      return 'vello';
    case RENDERER_VALUE_TINY_SKIA:
      return 'tiny-skia';
    default:
      return undefined;
  }
}

/**
 * レンダラ選択がなぜその backend になったか（`selected scene renderer:` ネイティブ
 * ログ相当の観測点）。ADR-0050 の `RendererSelectionReason` を web bundle 選択の粒度
 * に落とした語彙。
 */
export type BackendSelectionReason =
  | 'options-override'
  | 'query-override'
  | 'webgpu-primary'
  | 'webgpu-fallback'
  | 'webgpu-unavailable-skip';

export interface ResolvedCanvasBackend {
  backend: CanvasBackend;
  reason: BackendSelectionReason;
}

/**
 * Web の初回 boot でだけ試す候補。WebGPU が使える場合は Vello を試し、初期化に
 * 失敗した場合だけ tiny-skia へ進む。WebGPU が使えなければ tiny-skia から始める。
 */
export function resolveCanvasBackendAttemptOrder(
  options: ResolveCanvasBackendOptions | undefined,
  webgpuAvailable: boolean,
  search = '',
): ResolvedCanvasBackend[] {
  if (options?.backend !== undefined) {
    return [{ backend: options.backend, reason: 'options-override' }];
  }
  const forced = parseRendererQueryBackend(search);
  if (forced !== undefined) {
    return [{ backend: forced, reason: 'query-override' }];
  }

  if (webgpuAvailable) {
    return [
      { backend: 'vello', reason: 'webgpu-primary' },
      { backend: 'tiny-skia', reason: 'webgpu-fallback' },
    ];
  }
  return [{
    backend: 'tiny-skia',
    reason: 'webgpu-unavailable-skip',
  }];
}

/**
 * どの Canvas WASM バックエンド（Scene Renderer）を「なぜ」ロードするかを決める
 * Renderer Selection Policy。優先順位は 明示 override（`options.backend`）＞
 * クエリ強制（`?renderer=`）＞ WebGPU 自動判定。Render Host から分離し、host に
 * 埋め込んだ if 文連鎖にしない（Hayate CONTEXT）。`search` を渡すことで
 * `createHayateWebHost` 自体がディープリンク（`?renderer=vello`）に追従できる。
 */
export function resolveCanvasBackendSelection(
  options: ResolveCanvasBackendOptions | undefined,
  webgpuAvailable: boolean,
  search = '',
): ResolvedCanvasBackend {
  return resolveCanvasBackendAttemptOrder(options, webgpuAvailable, search)[0]!;
}

/**
 * {@link resolveCanvasBackendSelection} の backend だけを返す薄いラッパー。
 */
export function resolveCanvasBackend(
  options: ResolveCanvasBackendOptions | undefined,
  webgpuAvailable: boolean,
  search = '',
): CanvasBackend {
  return resolveCanvasBackendSelection(options, webgpuAvailable, search).backend;
}
