import type {
  CreateHayateWebHostOptions,
  FramePipelineObservation,
  WebHost,
} from '@torimi/hayate-host';
import type { Host, HostSession } from '@torimi/tsubame-app';
import { DomRenderer } from '@torimi/tsubame-renderer-dom';
import type { HayateRendererOptions } from '@torimi/tsubame-renderer-hayate';
import type { IRenderer } from '@torimi/tsubame-renderer-protocol';

export interface BrowserEnvironment {
  readonly search: string;
  readonly hasEditContext: boolean;
}

export type BrowserTarget = 'dom' | 'hayate';

export type TuningSource =
  | { readonly kind: 'none' }
  | { readonly kind: 'inline'; readonly json: string }
  | { readonly kind: 'optional-url'; readonly url: string };

export interface TuningResponse {
  readonly ok: boolean;
  text(): Promise<string>;
}

export type FetchTuning = (url: string) => Promise<TuningResponse>;
export type LoadTuning = (
  source: TuningSource,
) => string | undefined | Promise<string | undefined>;
export type CreateWebHost = (
  canvas: HTMLCanvasElement,
  options?: CreateHayateWebHostOptions,
) => Promise<WebHost>;

export interface StartedHayateRenderer extends IRenderer {
  start(): void;
  stop(): void;
}

export type CreateHayateRenderer = (
  options: HayateRendererOptions,
) => StartedHayateRenderer;

/** The deliberately narrow browser debug surface. Runtime resources never escape through it. */
export interface BrowserHostInspection {
  readonly target: BrowserTarget;
  pipelineObservation(): Promise<FramePipelineObservation | null>;
}

export interface BrowserHostHandle extends Host {
  inspection(): BrowserHostInspection;
}

export interface BrowserHostOptions {
  readonly dom: HTMLElement;
  readonly canvas: HTMLCanvasElement;
  readonly environment?: BrowserEnvironment;
  readonly tuning?: TuningSource;
  /** Test seam; production constructs the concrete DOM renderer. */
  readonly createDomRenderer?: (container: HTMLElement) => IRenderer;
  /** Test seams around the three independently fallible Hayate start stages. */
  readonly loadTuning?: LoadTuning;
  readonly createHayateWebHost?: CreateWebHost;
  readonly createHayateRenderer?: CreateHayateRenderer;
  /** Fetch seam used only by the default optional-url tuning loader. */
  readonly fetchTuning?: FetchTuning;
}

const DEFAULT_TUNING_SOURCE: TuningSource = { kind: 'none' };

function currentEnvironment(): BrowserEnvironment {
  return {
    search: globalThis.location?.search ?? '',
    hasEditContext: 'EditContext' in globalThis,
  };
}

function selectTarget(environment: BrowserEnvironment): BrowserTarget {
  return new URLSearchParams(environment.search).get('renderer') === 'dom' ||
    !environment.hasEditContext
    ? 'dom'
    : 'hayate';
}

class ResourceTransaction {
  private readonly cleanups: Array<() => void> = [];
  private disposed = false;

  defer(cleanup: () => void): void {
    if (this.disposed) {
      try {
        cleanup();
      } catch {
        // A late resource still belongs to this already-disposed transaction.
      }
      return;
    }
    this.cleanups.push(cleanup);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (let index = this.cleanups.length - 1; index >= 0; index -= 1) {
      try {
        this.cleanups[index]!();
      } catch {
        // Every registered cleanup gets its turn even when an earlier one fails.
      }
    }
    this.cleanups.length = 0;
  }
}

function registerVisibility(
  transaction: ResourceTransaction,
  dom: HTMLElement,
  canvas: HTMLCanvasElement,
  target: BrowserTarget,
): void {
  const domHidden = dom.hidden;
  const canvasHidden = canvas.hidden;
  transaction.defer(() => {
    dom.hidden = domHidden;
    canvas.hidden = canvasHidden;
  });
  dom.hidden = target === 'hayate';
  canvas.hidden = target === 'dom';
}

async function defaultLoadTuning(
  source: TuningSource,
  fetchTuning: FetchTuning | undefined,
): Promise<string | undefined> {
  if (source.kind === 'none') return undefined;
  if (source.kind === 'inline') return source.json;

  const request =
    fetchTuning ??
    (typeof globalThis.fetch === 'function'
      ? (url: string) => globalThis.fetch(url)
      : undefined);
  if (request === undefined) return undefined;
  try {
    const response = await request(source.url);
    return response.ok ? await response.text() : undefined;
  } catch {
    return undefined;
  }
}

async function defaultHayateFactories(): Promise<{
  createWebHost: CreateWebHost;
  createRenderer: CreateHayateRenderer;
}> {
  // DOM start never evaluates these browser runtime modules. Scene backend selection remains in
  // Hayate's Rust Render Host; this adapter treats every non-DOM query value as opaque.
  const [{ createHayateWebHost }, { HayateRenderer }] = await Promise.all([
    import('@torimi/hayate-host'),
    import('@torimi/tsubame-renderer-hayate'),
  ]);
  return {
    createWebHost: createHayateWebHost,
    createRenderer: (options) => new HayateRenderer(options),
  };
}

class BrowserHost implements BrowserHostHandle {
  private readonly target: BrowserTarget;
  private observePipeline: (() => Promise<FramePipelineObservation>) | undefined;

  constructor(private readonly options: BrowserHostOptions) {
    this.target = selectTarget(options.environment ?? currentEnvironment());
  }

  inspection(): BrowserHostInspection {
    return Object.freeze({
      target: this.target,
      pipelineObservation: () => {
        if (this.target === 'dom') return Promise.resolve(null);
        if (this.observePipeline === undefined) {
          return Promise.reject(new Error('Hayate Browser Host is not active'));
        }
        return this.observePipeline();
      },
    });
  }

  start(): HostSession | Promise<HostSession> {
    return this.target === 'dom' ? this.startDom() : this.startHayate();
  }

  private startDom(): HostSession {
    const { dom, canvas } = this.options;
    const transaction = new ResourceTransaction();
    registerVisibility(transaction, dom, canvas, this.target);
    try {
      const renderer = (this.options.createDomRenderer ??
        ((container) => new DomRenderer({ container })))(dom);
      return { renderer, dispose: () => transaction.dispose() };
    } catch (error) {
      transaction.dispose();
      throw error;
    }
  }

  private async startHayate(): Promise<HostSession> {
    const { dom, canvas } = this.options;
    const transaction = new ResourceTransaction();
    registerVisibility(transaction, dom, canvas, this.target);

    try {
      const source = this.options.tuning ?? DEFAULT_TUNING_SOURCE;
      const tuning = await (this.options.loadTuning ??
        ((value) => defaultLoadTuning(value, this.options.fetchTuning)))(source);

      const defaults =
        this.options.createHayateWebHost === undefined ||
        this.options.createHayateRenderer === undefined
          ? await defaultHayateFactories()
          : undefined;
      const createWebHost =
        this.options.createHayateWebHost ?? defaults!.createWebHost;
      const createRenderer =
        this.options.createHayateRenderer ?? defaults!.createRenderer;

      const webHost = await createWebHost(canvas, { tuning });
      const observePipeline = () => webHost.pipelineObservation();
      this.observePipeline = observePipeline;
      transaction.defer(() => {
        try {
          webHost.detach();
        } finally {
          if (this.observePipeline === observePipeline) {
            this.observePipeline = undefined;
          }
        }
      });

      const renderer = createRenderer({
        raw: webHost.raw,
        requestFrame: webHost.requestFrame,
        cancelFrame: webHost.cancelFrame,
      });
      transaction.defer(() => renderer.stop());
      renderer.start();

      return { renderer, dispose: () => transaction.dispose() };
    } catch (error) {
      transaction.dispose();
      throw error;
    }
  }
}

/** Create the browser App-layer implementation of Tsubame's runtime-blind Host port. */
export function createBrowserHost(options: BrowserHostOptions): BrowserHostHandle {
  return new BrowserHost(options);
}
