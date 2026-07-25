import manifest from '@torimi/hayate-protocol-spec/manifest' with { type: 'json' };
import {
  bootWorkerEngineBridge,
  createBrowserWorkerTransport,
  createWorkerInputProxy,
  WorkerBootError,
  type WorkerTransport,
} from './worker-boot.js';
import type {
  CanvasHandle,
  FramePipelineObservation,
  MainEditContextSink,
} from './worker-host.js';
import type { RawHayate } from './raw-hayate.js';

export type { RawHayate, HayateEffectiveVisual, HayateColorRecord } from './raw-hayate.js';
export { MainThreadShim, WorkerEngineDispatcher } from './worker-host.js';
export {
  bootWorkerEngineBridge,
  createWorkerInputProxy,
  createBrowserWorkerTransport,
  WORKER_ENTRY_URL,
  WORKER_NAME,
  WORKER_DETACH_TIMEOUT_MS,
  MIN_SURFACE_DIMENSION_PX,
  DEFAULT_DEVICE_PIXEL_RATIO,
  workerSurfaceMetrics,
  KEY_MODIFIER_SHIFT,
  KEY_MODIFIER_CTRL,
  KEY_MODIFIER_ALT,
  KEY_MODIFIER_META,
  WorkerBootError,
  type WorkerTransport,
  type WorkerBootFailureCode,
  type WorkerEngineBridgeHandle,
  type BootWorkerEngineBridgeOptions,
} from './worker-boot.js';
export type {
  CanvasHandle,
  FramePipelineObservation,
  ImePresentation,
  MainEditContextSink,
  MainToWorker,
  WorkerEngine,
  WorkerMutation,
  WorkerPointerKind,
  WorkerToMain,
} from './worker-host.js';
export {
  attachAccessibilityMirror,
  ACCESSKIT_ROLE_TO_ARIA,
  A11Y_ROOT_ATTR,
  A11Y_NODE_ID_PREFIX,
  MIRROR_OPACITY,
  MIRROR_POINTER_EVENTS,
  type DetachAccessibilityMirror,
  type AccessibilityMirror,
} from './accessibility-mirror.js';

/** Decoder version compiled into this host. */
export const HOST_PROTOCOL_VERSION: number = manifest.version;

/**
 * Canvas Mode host. The main/DOM thread owns only the clock and structured-clone transport;
 * OffscreenCanvas, Core, Render Host, selected Scene Renderer, and the shared frame pipeline all
 * live in one Worker.
 */
export interface WebHost {
  readonly raw: RawHayate;
  readonly requestFrame: (cb: FrameRequestCallback) => number;
  readonly cancelFrame: (handle: number) => void;
  readonly pipelineObservation: () => Promise<FramePipelineObservation>;
  readonly detach: () => void;
}

export interface CreateHayateWebHostOptions {
  /** Development-only tuning JSON applied inside the Worker before the first app frame. */
  tuning?: string;
  /** Clock injection seam. Rendering still executes in the Worker. */
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
  /** Transport injection seam. Production creates exactly one module Worker. */
  spawnWorker?: () => WorkerTransport;
  /** Main-thread EditContext sink (ADR-0069). */
  imeSink?: MainEditContextSink;
  /** OffscreenCanvas transfer injection seam. */
  transferControlToOffscreen?: (canvas: HTMLCanvasElement) => CanvasHandle;
}

/** Worker IME presentation is the only engine fact projected back onto the DOM thread. */
function createMainEditContextSink(canvas: HTMLCanvasElement): MainEditContextSink {
  const EditContextCtor = (
    globalThis as unknown as { EditContext?: new () => object }
  ).EditContext;
  if (!EditContextCtor) {
    return {
      setKeyboardVisible: () => {},
      setCaretRect: () => {},
    };
  }
  const context = new EditContextCtor() as {
    updateControlBounds?(rect: DOMRect): void;
    updateSelectionBounds?(rect: DOMRect): void;
  };
  return {
    setKeyboardVisible: (visible) => {
      Reflect.set(canvas, 'editContext', visible ? context : null);
    },
    setCaretRect: (caret) => {
      if (!caret || typeof DOMRect === 'undefined') return;
      const surface = canvas.getBoundingClientRect();
      const rect = new DOMRect(
        surface.left + caret.x,
        surface.top + caret.y,
        caret.width,
        caret.height,
      );
      context.updateControlBounds?.(rect);
      context.updateSelectionBounds?.(rect);
    },
  };
}

/**
 * Start the sole Canvas Mode execution path. Missing Worker/OffscreenCanvas capabilities and
 * renderer initialization errors remain typed boot failures; no main-thread Canvas renderer is
 * constructed as a fallback.
 */
export async function createHayateWebHost(
  canvas: HTMLCanvasElement,
  options: CreateHayateWebHostOptions = {},
): Promise<WebHost> {
  const transport = (options.spawnWorker ?? createBrowserWorkerTransport)();
  const transferControlToOffscreen =
    options.transferControlToOffscreen ??
    ((surface: HTMLCanvasElement) => {
      const transfer = (
        surface as HTMLCanvasElement & {
          transferControlToOffscreen?: () => CanvasHandle;
        }
      ).transferControlToOffscreen;
      if (typeof transfer !== 'function') {
        throw new WorkerBootError(
          'offscreen-canvas-unavailable',
          'transferControlToOffscreen is unavailable',
        );
      }
      return transfer.call(surface);
    });
  const dpr =
    typeof globalThis.devicePixelRatio === 'number'
      ? globalThis.devicePixelRatio
      : 1;

  let bridge;
  try {
    bridge = bootWorkerEngineBridge(canvas, {
      transport,
      ime: options.imeSink ?? createMainEditContextSink(canvas),
      transferControlToOffscreen,
      dpr,
    });
    await bridge.ready;
  } catch (error) {
    transport.terminate();
    if (error instanceof WorkerBootError) throw error;
    throw new WorkerBootError(
      'offscreen-canvas-unavailable',
      error instanceof Error ? error.message : String(error),
    );
  }

  const raw = createWorkerInputProxy(bridge.shim);
  if (options.tuning != null) raw.set_tuning(options.tuning);

  return {
    raw,
    requestFrame:
      options.requestFrame ??
      ((cb: FrameRequestCallback) => globalThis.requestAnimationFrame(cb)),
    cancelFrame:
      options.cancelFrame ??
      ((handle: number) => globalThis.cancelAnimationFrame(handle)),
    pipelineObservation: bridge.pipelineObservation,
    detach: bridge.detach,
  };
}
