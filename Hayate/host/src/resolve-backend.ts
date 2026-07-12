export type CanvasBackend = 'vello' | 'tiny-skia' | 'vello-cpu';

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
export const RENDERER_VALUE_VELLO_CPU = 'vello-cpu';

/**
 * `?renderer=vello|tiny-skia|vello-cpu` を canvas backend の強制指定として解釈する。
 * `auto` / `dom` / 未知値 / 未指定は canvas backend の強制ではないので `undefined`
 * （＝ WebGPU プローブ結果に委ねる）。`dom` の DOM/Canvas モード判定は Tsubame の
 * `detectMode` が別軸で持つ（host は canvas backend のみ扱う）。
 */
export function parseRendererQueryBackend(search: string): CanvasBackend | undefined {
  const value = new URLSearchParams(search).get(RENDERER_QUERY_PARAM);
  switch (value) {
    case RENDERER_VALUE_VELLO:
      return 'vello';
    case RENDERER_VALUE_TINY_SKIA:
      return 'tiny-skia';
    case RENDERER_VALUE_VELLO_CPU:
      return 'vello-cpu';
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
  | 'webgpu-auto'
  | 'webgpu-unavailable-fallback';

export interface ResolvedCanvasBackend {
  backend: CanvasBackend;
  reason: BackendSelectionReason;
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
  if (options?.backend !== undefined) {
    return { backend: options.backend, reason: 'options-override' };
  }
  const forced = parseRendererQueryBackend(search);
  if (forced !== undefined) {
    return { backend: forced, reason: 'query-override' };
  }
  return webgpuAvailable
    ? { backend: 'vello', reason: 'webgpu-auto' }
    : { backend: 'tiny-skia', reason: 'webgpu-unavailable-fallback' };
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
