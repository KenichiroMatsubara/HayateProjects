import type { Host, HostSession } from '@torimi/tsubame-app';
import { DomRenderer } from '@torimi/tsubame-renderer-dom';
import type { IRenderer } from '@torimi/tsubame-renderer-protocol';

export interface BrowserEnvironment {
  readonly search: string;
  readonly hasEditContext: boolean;
}

export type StartHayate = (
  canvas: HTMLCanvasElement,
) => HostSession | Promise<HostSession>;

export interface BrowserHostOptions {
  readonly dom: HTMLElement;
  readonly canvas: HTMLCanvasElement;
  readonly environment?: BrowserEnvironment;
  /** Test seam; production constructs the concrete DOM renderer. */
  readonly createDomRenderer?: (container: HTMLElement) => IRenderer;
  /** Test seam; production dynamically loads the Hayate runtime only for the Hayate target. */
  readonly startHayate?: StartHayate;
}

function currentEnvironment(): BrowserEnvironment {
  return {
    search: globalThis.location?.search ?? '',
    hasEditContext: 'EditContext' in globalThis,
  };
}

function selectDom(environment: BrowserEnvironment): boolean {
  return (
    new URLSearchParams(environment.search).get('renderer') === 'dom' ||
    !environment.hasEditContext
  );
}

function visibilityRestore(
  dom: HTMLElement,
  canvas: HTMLCanvasElement,
): () => void {
  const domHidden = dom.hidden;
  const canvasHidden = canvas.hidden;
  let restored = false;
  return () => {
    if (restored) return;
    restored = true;
    dom.hidden = domHidden;
    canvas.hidden = canvasHidden;
  };
}

function wrapSession(session: HostSession, restore: () => void): HostSession {
  let disposed = false;
  return {
    renderer: session.renderer,
    dispose() {
      if (disposed) return;
      disposed = true;
      try {
        session.dispose();
      } finally {
        restore();
      }
    },
  };
}

async function startDefaultHayate(canvas: HTMLCanvasElement): Promise<HostSession> {
  // Keep both modules outside the DOM evaluation path. Scene backend selection remains inside
  // Hayate's Rust Render Host; this adapter never interprets non-DOM renderer query values.
  const [{ createHayateWebHost }, { HayateRenderer }] = await Promise.all([
    import('@torimi/hayate-host'),
    import('@torimi/tsubame-renderer-hayate'),
  ]);
  const webHost = await createHayateWebHost(canvas);
  const renderer = new HayateRenderer({
    raw: webHost.raw,
    requestFrame: webHost.requestFrame,
    cancelFrame: webHost.cancelFrame,
  });
  renderer.start();
  let disposed = false;
  return {
    renderer,
    dispose() {
      if (disposed) return;
      disposed = true;
      try {
        renderer.stop();
      } finally {
        webHost.detach();
      }
    },
  };
}

class BrowserHost implements Host {
  constructor(private readonly options: BrowserHostOptions) {}

  start(): HostSession | Promise<HostSession> {
    const { dom, canvas } = this.options;
    const environment = this.options.environment ?? currentEnvironment();
    const restore = visibilityRestore(dom, canvas);

    if (selectDom(environment)) {
      dom.hidden = false;
      canvas.hidden = true;
      try {
        const renderer = (this.options.createDomRenderer ??
          ((container) => new DomRenderer({ container })))(dom);
        return wrapSession({ renderer, dispose() {} }, restore);
      } catch (error) {
        restore();
        throw error;
      }
    }

    dom.hidden = true;
    canvas.hidden = false;
    try {
      const started = (this.options.startHayate ?? startDefaultHayate)(canvas);
      if (started instanceof Promise) {
        return started.then(
          (session) => wrapSession(session, restore),
          (error: unknown) => {
            restore();
            throw error;
          },
        );
      }
      return wrapSession(started, restore);
    } catch (error) {
      restore();
      throw error;
    }
  }
}

/** Create the browser App-layer implementation of Tsubame's runtime-blind Host port. */
export function createBrowserHost(options: BrowserHostOptions): Host {
  return new BrowserHost(options);
}
