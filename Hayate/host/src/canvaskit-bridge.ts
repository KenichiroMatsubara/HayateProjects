import CanvasKitInit, { type CanvasKit, type Surface } from 'canvaskit-wasm';
import canvasKitWasmAssetUrl from 'canvaskit-wasm/bin/canvaskit.wasm?url';

/** Opaque Host-owned bridge looked up by the CanvasKit WASM adapter. */
export const CANVASKIT_BRIDGE_KEY = '__hayateCanvasKitBridge';

const CLEAR = 0;
const FILL_RECT = 1;

interface CanvasKitSurface {
  readonly canvasKit: CanvasKit;
  readonly surface: Surface;
}

interface CanvasKitBridge {
  replay(canvas: HTMLCanvasElement, commands: Float32Array): void;
  resize(canvas: HTMLCanvasElement): void;
}

type CanvasKitInitializer = typeof CanvasKitInit;

let canvasKitPromise: Promise<CanvasKit> | undefined;
let surfaces = new WeakMap<HTMLCanvasElement, CanvasKitSurface>();

function canvasKitWasmUrl(file: string): string {
  if (file !== 'canvaskit.wasm') {
    throw new Error(`CanvasKit requested an unsupported asset: ${file}`);
  }
  return canvasKitWasmAssetUrl;
}

async function loadCanvasKit(initialize: CanvasKitInitializer): Promise<CanvasKit> {
  canvasKitPromise ??= initialize({ locateFile: canvasKitWasmUrl });
  return canvasKitPromise;
}

function createSurface(canvas: HTMLCanvasElement, canvasKit: CanvasKit): CanvasKitSurface {
  const surface = canvasKit.MakeWebGLCanvasSurface(canvas);
  if (surface == null) throw new Error('CanvasKit surface unavailable');
  return { canvasKit, surface };
}

function requireSurface(canvas: HTMLCanvasElement): CanvasKitSurface {
  const resource = surfaces.get(canvas);
  if (!resource) throw new Error('CanvasKit surface was not prepared for this canvas');
  return resource;
}

function replay(canvas: HTMLCanvasElement, commands: Float32Array): void {
  const { canvasKit, surface } = requireSurface(canvas);
  const skCanvas = surface.getCanvas();
  const paint = new canvasKit.Paint();

  try {
    for (let offset = 0; offset < commands.length;) {
      const opcode = commands[offset++]!;
      if (opcode === CLEAR) {
        if (offset + 4 > commands.length) throw new Error('CanvasKit command buffer: truncated clear');
        skCanvas.clear(canvasKit.Color4f(
          commands[offset++]!,
          commands[offset++]!,
          commands[offset++]!,
          commands[offset++]!,
        ));
        continue;
      }
      if (opcode === FILL_RECT) {
        if (offset + 9 > commands.length) throw new Error('CanvasKit command buffer: truncated fillRect');
        const x = commands[offset++]!;
        const y = commands[offset++]!;
        const width = commands[offset++]!;
        const height = commands[offset++]!;
        const color = canvasKit.Color4f(
          commands[offset++]!,
          commands[offset++]!,
          commands[offset++]!,
          commands[offset++]!,
        );
        const cornerRadius = commands[offset++]!;
        const rect = canvasKit.LTRBRect(x, y, x + width, y + height);
        paint.setColor(color);
        if (cornerRadius > 0) {
          skCanvas.drawRRect(canvasKit.RRectXY(rect, cornerRadius, cornerRadius), paint);
        } else {
          skCanvas.drawRect(rect, paint);
        }
        continue;
      }
      throw new Error(`CanvasKit command buffer: unknown opcode ${opcode}`);
    }
    surface.flush();
  } finally {
    paint.delete();
  }
}

function resize(canvas: HTMLCanvasElement): void {
  const previous = requireSurface(canvas);
  previous.surface.delete();
  surfaces.set(canvas, createSurface(canvas, previous.canvasKit));
}

function installBridge(): void {
  const target = globalThis as Record<string, unknown>;
  if (target[CANVASKIT_BRIDGE_KEY]) return;
  const bridge: CanvasKitBridge = { replay, resize };
  target[CANVASKIT_BRIDGE_KEY] = bridge;
}

/**
 * Initializes the Host-owned CanvasKit surface before its WASM Scene Renderer is imported.
 * Rust only receives the canvas and calls the opaque replay bridge once per frame.
 */
export async function prepareCanvasKitSurface(
  canvas: HTMLCanvasElement,
  initialize: CanvasKitInitializer = CanvasKitInit,
): Promise<void> {
  installBridge();
  if (surfaces.has(canvas)) return;
  surfaces.set(canvas, createSurface(canvas, await loadCanvasKit(initialize)));
}

/** Test-only reset for the Host-owned singleton state. */
export function resetCanvasKitBridgeForTesting(): void {
  canvasKitPromise = undefined;
  surfaces = new WeakMap<HTMLCanvasElement, CanvasKitSurface>();
  delete (globalThis as Record<string, unknown>)[CANVASKIT_BRIDGE_KEY];
}
