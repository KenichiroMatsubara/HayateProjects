import type { CreateHayateWebHostOptions, WebHost } from '@hayate/host';

export type { WebHost } from '@hayate/host';

/**
 * App Bundle が `eval` 時に登録する mount。host bootstrap（`raw` + frame-clock）を受け取り、
 * バンドル側が持ち込む `new CanvasRenderer({ raw, requestFrame, cancelFrame })` と FW で
 * App を mount する（ADR-0001：renderer-canvas も FW もバンドルが持つ）。
 */
export type MiharashiMount = (host: WebHost) => void;

/**
 * App Bundle が mount を露出する global プロパティ名。native（`globalThis.__tsubame`,
 * ADR-0112）と対称の、バンドル → ホストの受け渡しシーム。
 */
export const MIHARASHI_MOUNT_GLOBAL = '__miharashiMount';

/**
 * dev-server がバンドルを配信する HTTP ルート。`@miharashi/dev-server` の `BUNDLE_ROUTE`
 * と一致させる wire 契約（node 依存を web ホストへ持ち込まないため値で複製する）。
 */
export const DEFAULT_BUNDLE_ROUTE = '/bundle.js';

/** バンドル fetch のタイムアウト。応答しない dev-server で永久に待たないための上限。 */
const BUNDLE_FETCH_TIMEOUT_MS = 10_000;

export interface BootMiharashiHostOptions {
  /** バンドルを取得する dev-server の origin（例 `http://127.0.0.1:5179`）。 */
  readonly devServerUrl: string;
  /** mount 先の surface。`createHayateWebHost` がこの上に raw を確立する。 */
  readonly canvas: HTMLCanvasElement;
  /** dev-server 上のバンドルルート。既定は {@link DEFAULT_BUNDLE_ROUTE}。 */
  readonly bundleRoute?: string;
  /** `createHayateWebHost` に渡す backend / tuning 等。auto モードでは省略可。 */
  readonly hostOptions?: CreateHayateWebHostOptions;

  // ── テスト注入 seam ──────────────────────────────────────────────────────
  /** バンドル text を取得する seam。既定は timeout 付き `fetch`。 */
  readonly fetchBundle?: (bundleUrl: string) => Promise<string>;
  /** バンドル text を eval して mount を取り出す seam。既定は indirect eval + global 読み。 */
  readonly evalBundle?: (source: string) => MiharashiMount;
  /** host bootstrap を確立する seam。既定は `@hayate/host` の `createHayateWebHost`。 */
  readonly createHost?: (
    canvas: HTMLCanvasElement,
    options?: CreateHayateWebHostOptions,
  ) => Promise<WebHost>;
}

/** タイムアウト付き `fetch` でバンドル text を取得する既定実装。 */
async function defaultFetchBundle(bundleUrl: string): Promise<string> {
  const res = await fetch(bundleUrl, { signal: AbortSignal.timeout(BUNDLE_FETCH_TIMEOUT_MS) });
  if (!res.ok) {
    throw new Error(`Miharashi: バンドル取得に失敗しました（${res.status} ${bundleUrl}）`);
  }
  return res.text();
}

/**
 * バンドル text を global scope で indirect eval し、登録された mount を取り出す既定実装。
 * バンドルは IIFE として `globalThis.__miharashiMount` を立てる（ADR-0112 の単一 IIFE と同型）。
 */
function defaultEvalBundle(source: string): MiharashiMount {
  // indirect eval（`(0, eval)`）で global scope に評価する。バンドルの副作用で global mount が立つ。
  (0, eval)(source);
  const mount = (globalThis as Record<string, unknown>)[MIHARASHI_MOUNT_GLOBAL];
  if (typeof mount !== 'function') {
    throw new Error(
      `Miharashi: バンドルが ${MIHARASHI_MOUNT_GLOBAL} を露出していません（mount 契約違反）`,
    );
  }
  return mount as MiharashiMount;
}

/** `@hayate/host` の `createHayateWebHost` を遅延 import する既定実装。 */
async function defaultCreateHost(
  canvas: HTMLCanvasElement,
  options?: CreateHayateWebHostOptions,
): Promise<WebHost> {
  const { createHayateWebHost } = await import('@hayate/host');
  return createHayateWebHost(canvas, options);
}

/**
 * Miharashi Web ホストを起動する：dev-server からバンドルを fetch → eval し、
 * `createHayateWebHost(canvas)` で host bootstrap（`raw` / `requestFrame` / `cancelFrame`）を
 * 確立して、バンドルが露出した mount に渡す。ホストは FW も `@tsubame/renderer-canvas` も
 * 持たず、それらは流し込むバンドル側が持ち込む（ADR-0001）。
 */
export async function bootMiharashiHost(options: BootMiharashiHostOptions): Promise<void> {
  const fetchBundle = options.fetchBundle ?? defaultFetchBundle;
  const evalBundle = options.evalBundle ?? defaultEvalBundle;
  const createHost = options.createHost ?? defaultCreateHost;
  const bundleRoute = options.bundleRoute ?? DEFAULT_BUNDLE_ROUTE;

  const bundleUrl = new URL(bundleRoute, options.devServerUrl).href;
  const source = await fetchBundle(bundleUrl);
  const mount = evalBundle(source);
  const host = await createHost(options.canvas, options.hostOptions);
  mount(host);
}
