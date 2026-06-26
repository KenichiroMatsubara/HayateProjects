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

// ── full reload ループ（ホスト側）────────────────────────────────────────────

/**
 * dev-server が reload シグナルを流す WS ルート。`@miharashi/dev-server` の `RELOAD_ROUTE` と
 * 一致させる wire 契約（node 依存を web ホストへ持ち込まないため値で複製する）。
 */
export const DEFAULT_RELOAD_ROUTE = '/reload';

/** ホストに full reload を促す WS メッセージ本文。`@miharashi/dev-server` の `RELOAD_MESSAGE` と一致。 */
export const RELOAD_MESSAGE = 'reload';

/**
 * WS 切断後に再接続するまでの待ち時間（ms）。dev-server 再起動・ネットワーク瞬断後に繋ぎ直す。
 * **プレースホルダ値**（実値調整は #8, ADR-0001）。
 */
const WS_RECONNECT_BACKOFF_MS = 1_000;

/**
 * reload シグナルを運ぶ WS への最小ポート。既定は `WebSocket` を包む薄いアダプタだが、テストは
 * これを注入して実 WS / 実ブラウザを巻き込まずに配線を観測する。
 */
export interface ReloadSocket {
  /** テキストメッセージ受信時のコールバックを登録する。 */
  onMessage(cb: (data: string) => void): void;
  /** 切断時のコールバックを登録する。 */
  onClose(cb: () => void): void;
  /** 接続を閉じる。 */
  close(): void;
}

export interface SubscribeReloadOptions {
  /** dev-server の origin（例 `http://127.0.0.1:5181`）。WS スキームへ読み替えて繋ぐ。 */
  readonly devServerUrl: string;
  /** `reload` 受信時に呼ぶ。ホストはここで full reload（再 fetch → 再 mount）を起こす。 */
  readonly onReload: () => void;
  /** dev-server 上の reload ルート。既定は {@link DEFAULT_RELOAD_ROUTE}。 */
  readonly reloadRoute?: string;

  // ── テスト注入 seam ──────────────────────────────────────────────────────
  /** WS を張る seam。既定は `WebSocket` を {@link ReloadSocket} に包む。 */
  readonly connect?: (wsUrl: string) => ReloadSocket;
  /** 再接続の遅延スケジュール seam。既定は `setTimeout`。 */
  readonly scheduleReconnect?: (fn: () => void, delayMs: number) => void;
}

export interface ReloadSubscription {
  /** 購読を止める：WS を閉じ、以後の再接続も行わない。 */
  close(): void;
}

/** `WebSocket` を {@link ReloadSocket} に包む既定実装。 */
function defaultConnect(wsUrl: string): ReloadSocket {
  const ws = new WebSocket(wsUrl);
  return {
    onMessage(cb) {
      ws.addEventListener('message', (ev) => cb(String((ev as MessageEvent).data)));
    },
    onClose(cb) {
      ws.addEventListener('close', () => cb());
    },
    close() {
      ws.close();
    },
  };
}

/** `setTimeout` で再接続を遅延する既定実装。 */
function defaultScheduleReconnect(fn: () => void, delayMs: number): void {
  setTimeout(fn, delayMs);
}

/**
 * dev-server の reload WS を購読し、`reload` 受信ごとに {@link SubscribeReloadOptions.onReload} を
 * 起こす。切断時は名前付き backoff（{@link WS_RECONNECT_BACKOFF_MS}）で繋ぎ直す（dev-server 再起動・
 * 瞬断に耐える）。ホスト側は WS を JS に中継するだけで、reload の意味づけ（再 fetch → 再 mount）は
 * `onReload` が担う（ADR-0001：ホストのネイティブ契約は full reload / HMR で不変）。
 */
export function subscribeReload(options: SubscribeReloadOptions): ReloadSubscription {
  const connect = options.connect ?? defaultConnect;
  const scheduleReconnect = options.scheduleReconnect ?? defaultScheduleReconnect;
  const reloadRoute = options.reloadRoute ?? DEFAULT_RELOAD_ROUTE;
  const wsUrl = new URL(reloadRoute, options.devServerUrl).href.replace(/^http/, 'ws');

  let stopped = false;
  let socket: ReloadSocket | undefined;

  const open = (): void => {
    if (stopped) return;
    socket = connect(wsUrl);
    socket.onMessage((data) => {
      if (data === RELOAD_MESSAGE) options.onReload();
    });
    socket.onClose(() => {
      if (stopped) return;
      scheduleReconnect(open, WS_RECONNECT_BACKOFF_MS);
    });
  };

  open();

  return {
    close() {
      stopped = true;
      socket?.close();
    },
  };
}

// ── full reload の合成ルート（ホスト側）──────────────────────────────────────

export interface StartMiharashiHostOptions {
  /** バンドルを取得し reload を購読する dev-server の origin（例 `http://127.0.0.1:5181`）。 */
  readonly devServerUrl: string;
  /**
   * mount 先の surface を用意する。full reload は呼ぶたびに**新しい canvas**を返して、
   * レンダラ初期化と state を完全にやり直す（state は飛ぶ・ADR-0001 / CONTEXT.md「Reload」）。
   * canvas のコンテキスト型は一度決まると変えられないため、再 mount には新しい surface が要る。
   */
  readonly acquireCanvas: () => HTMLCanvasElement;
  /** `createHayateWebHost` に渡す backend / tuning 等。auto モードでは省略可。 */
  readonly hostOptions?: CreateHayateWebHostOptions;
  /** dev-server 上の reload ルート。既定は {@link DEFAULT_RELOAD_ROUTE}。 */
  readonly reloadRoute?: string;
  /** boot 完了 / 失敗の通知。e2e / デバッグが mount 到達を観測できるようにする。 */
  readonly onBootSettled?: (result: { ok: true } | { ok: false; error: unknown }) => void;

  // ── テスト注入 seam ──────────────────────────────────────────────────────
  /** 1 回分の fetch → eval → host → mount を行う seam。既定は {@link bootMiharashiHost}。 */
  readonly boot?: (options: BootMiharashiHostOptions) => Promise<void>;
  /** reload 購読を張る seam。既定は {@link subscribeReload}。 */
  readonly subscribe?: (options: SubscribeReloadOptions) => ReloadSubscription;
}

export interface MiharashiHostHandle {
  /** ホストを止める：reload 購読を解除する。 */
  close(): void;
}

/**
 * Miharashi Web ホストの full reload ループを起動する。初回に一度 boot（fetch → eval →
 * host bootstrap → mount）し、dev-server からの `reload` を購読して、受信ごとに**新しい surface**
 * で再 boot する（full reload。state は飛ぶ）。ホストは reload の意味づけを `onReload` で閉じ込め、
 * ネイティブ契約（host bootstrap）は full reload / HMR で不変に保つ（ADR-0001）。
 */
export function startMiharashiHost(options: StartMiharashiHostOptions): MiharashiHostHandle {
  const boot = options.boot ?? bootMiharashiHost;
  const subscribe = options.subscribe ?? subscribeReload;

  const bootOnce = (): void => {
    boot({
      devServerUrl: options.devServerUrl,
      canvas: options.acquireCanvas(),
      hostOptions: options.hostOptions,
    }).then(
      () => options.onBootSettled?.({ ok: true }),
      (error: unknown) => options.onBootSettled?.({ ok: false, error }),
    );
  };

  bootOnce();

  const subscription = subscribe({
    devServerUrl: options.devServerUrl,
    reloadRoute: options.reloadRoute,
    onReload: bootOnce,
  });

  return {
    close() {
      subscription.close();
    },
  };
}
