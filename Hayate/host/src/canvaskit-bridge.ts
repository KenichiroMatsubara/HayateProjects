import CanvasKitInit, {
  type CanvasKit,
  type Image,
  type Surface,
  type Typeface,
} from 'canvaskit-wasm';
import canvasKitWasmAssetUrl from 'canvaskit-wasm/bin/canvaskit.wasm?url';

/** Opaque Host-owned bridge looked up by the CanvasKit WASM adapter. */
export const CANVASKIT_BRIDGE_KEY = '__hayateCanvasKitBridge';

const CLEAR = 0;
const FILL_RECT = 1;
const FILL_ROUNDED_RING = 2;
const DASHED_BORDER = 3;
const FILL_PATH = 4;
const STROKE_PATH = 5;
const DRAW_TEXT = 6;
const DRAW_IMAGE = 7;
const PUSH_TRANSFORM = 8;
const POP_TRANSFORM = 9;
const PUSH_CLIP_RECT = 10;
const PUSH_CLIP_PATH = 11;
const POP_CLIP = 12;
const BLURRED_RECT = 13;
const INSET_BLURRED_RECT = 14;

type ReplayErrorCategory = 'contract' | 'environment';

export class CanvasKitReplayError extends Error {
  constructor(readonly category: ReplayErrorCategory, message: string) {
    super(message);
    this.name = 'CanvasKitReplayError';
  }
}

type CanvasKitResource =
  | { kind: 'font'; id: number; bytes: Uint8Array }
  | {
    kind: 'image';
    id: number;
    width: number;
    height: number;
    alphaType: number;
    bytes: Uint8Array;
  };

interface CanvasKitSurface {
  readonly canvasKit: CanvasKit;
  readonly surface: Surface;
  readonly fonts: Map<number, Typeface>;
  readonly images: Map<number, Image>;
}

interface CanvasKitBridge {
  replay(
    canvas: HTMLCanvasElement,
    commands: Float32Array,
    resources?: CanvasKitResource[],
  ): void;
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
  return { canvasKit, surface, fonts: new Map(), images: new Map() };
}

function requireSurface(canvas: HTMLCanvasElement): CanvasKitSurface {
  const resource = surfaces.get(canvas);
  if (!resource) throw new Error('CanvasKit surface was not prepared for this canvas');
  return resource;
}

function replay(
  canvas: HTMLCanvasElement,
  commands: Float32Array,
  resources: CanvasKitResource[] = [],
): void {
  const resourceCache = requireSurface(canvas);
  const { canvasKit, surface } = resourceCache;
  registerResources(resourceCache, resources);
  const skCanvas = surface.getCanvas();
  const paint = new canvasKit.Paint();
  let offset = 0;

  const take = (count: number, command: string): number[] => {
    if (offset + count > commands.length) {
      throw new CanvasKitReplayError('contract', `CanvasKit command buffer: truncated ${command}`);
    }
    const values = Array.from(commands.subarray(offset, offset + count));
    offset += count;
    return values;
  };

  const color = (command: string) => canvasKit.Color4f(...take(4, command) as [number, number, number, number]);
  const rect = (x: number, y: number, width: number, height: number) =>
    canvasKit.LTRBRect(x, y, x + width, y + height);
  const setFill = (value: ReturnType<typeof canvasKit.Color4f>) => {
    paint.setStyle(canvasKit.PaintStyle.Fill);
    paint.setColor(value);
  };

  try {
    while (offset < commands.length) {
      const opcode = commands[offset++]!;
      if (opcode === CLEAR) {
        skCanvas.clear(color('clear'));
        continue;
      }
      if (opcode === FILL_RECT) {
        const [x, y, width, height] = take(4, 'fillRect bounds');
        const fillColor = color('fillRect color');
        const [cornerRadius] = take(1, 'fillRect radius');
        const bounds = rect(x!, y!, width!, height!);
        setFill(fillColor);
        if (cornerRadius! > 0) {
          skCanvas.drawRRect(canvasKit.RRectXY(bounds, cornerRadius!, cornerRadius!), paint);
        } else {
          skCanvas.drawRect(bounds, paint);
        }
        continue;
      }
      if (opcode === FILL_ROUNDED_RING || opcode === DASHED_BORDER) {
        const [x, y, width, height, radius, borderWidth] = take(6, 'ring');
        const ringColor = color('ring color');
        const outer = canvasKit.RRectXY(rect(x!, y!, width!, height!), radius!, radius!);
        setFill(ringColor);
        if (opcode === FILL_ROUNDED_RING) {
          const inset = borderWidth!;
          const inner = canvasKit.RRectXY(
            rect(x! + inset, y! + inset, Math.max(0, width! - inset * 2), Math.max(0, height! - inset * 2)),
            Math.max(0, radius! - inset),
            Math.max(0, radius! - inset),
          );
          skCanvas.drawDRRect(outer, inner, paint);
        } else {
          paint.setStyle(canvasKit.PaintStyle.Stroke);
          paint.setStrokeWidth(borderWidth!);
          const dash = canvasKit.PathEffect.MakeDash([borderWidth! * 3, borderWidth! * 2], 0);
          paint.setPathEffect(dash);
          skCanvas.drawRRect(outer, paint);
          paint.setPathEffect(null);
          dash.delete();
        }
        continue;
      }
      if (opcode === FILL_PATH || opcode === STROKE_PATH) {
        const pathColor = color('path color');
        if (opcode === FILL_PATH) {
          const [fillRule] = take(1, 'fill path rule');
          const path = readPath(canvasKit, commands, { get offset() { return offset; }, set offset(value) { offset = value; } });
          path.setFillType(fillRule === 1 ? canvasKit.FillType.EvenOdd : canvasKit.FillType.Winding);
          setFill(pathColor);
          skCanvas.drawPath(path, paint);
          path.delete();
        } else {
          const [width, cap, join, miter, dashCount] = take(5, 'stroke style');
          const dash = take(dashCount!, 'stroke dash');
          const [dashOffset] = take(1, 'stroke dash offset');
          const path = readPath(canvasKit, commands, { get offset() { return offset; }, set offset(value) { offset = value; } });
          paint.setStyle(canvasKit.PaintStyle.Stroke);
          paint.setColor(pathColor);
          paint.setStrokeWidth(width!);
          paint.setStrokeCap([canvasKit.StrokeCap.Butt, canvasKit.StrokeCap.Round, canvasKit.StrokeCap.Square][cap!]!);
          paint.setStrokeJoin([canvasKit.StrokeJoin.Miter, canvasKit.StrokeJoin.Round, canvasKit.StrokeJoin.Bevel][join!]!);
          paint.setStrokeMiter(miter!);
          const effect = dash.length > 0 ? canvasKit.PathEffect.MakeDash(dash, dashOffset!) : null;
          paint.setPathEffect(effect);
          skCanvas.drawPath(path, paint);
          paint.setPathEffect(null);
          effect?.delete();
          path.delete();
        }
        continue;
      }
      if (opcode === DRAW_TEXT) {
        const [id, x, y, size] = take(4, 'text header');
        const textColor = color('text color');
        const [glyphCount] = take(1, 'glyph count');
        const glyphs = new Uint16Array(glyphCount!);
        const positions = new Float32Array(glyphCount! * 2);
        for (let index = 0; index < glyphCount!; index += 1) {
          const [glyph, gx, gy] = take(3, 'glyph');
          glyphs[index] = glyph!;
          positions[index * 2] = gx!;
          positions[index * 2 + 1] = gy!;
        }
        const typeface = resourceCache.fonts.get(id!);
        if (!typeface) throw new CanvasKitReplayError('contract', `CanvasKit font resource ${id} is unresolved`);
        const font = new canvasKit.Font(typeface, size!);
        setFill(textColor);
        skCanvas.drawGlyphs(glyphs, positions, x!, y!, font, paint);
        font.delete();
        continue;
      }
      if (opcode === DRAW_IMAGE) {
        const [id, x, y, width, height] = take(5, 'image');
        const image = resourceCache.images.get(id!);
        if (!image) throw new CanvasKitReplayError('contract', `CanvasKit image resource ${id} is unresolved`);
        skCanvas.drawImageRect(
          image,
          canvasKit.LTRBRect(0, 0, image.width(), image.height()),
          rect(x!, y!, width!, height!),
          paint,
        );
        continue;
      }
      if (opcode === PUSH_TRANSFORM) {
        const [a, b, c, d, e, f] = take(6, 'transform');
        skCanvas.save();
        skCanvas.concat([a!, c!, e!, b!, d!, f!, 0, 0, 1]);
        continue;
      }
      if (opcode === POP_TRANSFORM || opcode === POP_CLIP) {
        skCanvas.restore();
        continue;
      }
      if (opcode === PUSH_CLIP_RECT) {
        const [x, y, width, height, tl, tr, br, bl] = take(8, 'clip rect');
        skCanvas.save();
        const bounds = rect(x!, y!, width!, height!);
        if (tl === tr && tr === br && br === bl) {
          skCanvas.clipRRect(canvasKit.RRectXY(bounds, tl!, tl!), canvasKit.ClipOp.Intersect, true);
        } else {
          const builder = new canvasKit.PathBuilder();
          builder.addRRect(canvasKit.RRectXY(bounds, Math.max(tl!, tr!, br!, bl!), Math.max(tl!, tr!, br!, bl!)));
          const path = builder.detachAndDelete();
          skCanvas.clipPath(path, canvasKit.ClipOp.Intersect, true);
          path.delete();
        }
        continue;
      }
      if (opcode === PUSH_CLIP_PATH) {
        const path = readPath(canvasKit, commands, { get offset() { return offset; }, set offset(value) { offset = value; } });
        skCanvas.save();
        skCanvas.clipPath(path, canvasKit.ClipOp.Intersect, true);
        path.delete();
        continue;
      }
      if (opcode === BLURRED_RECT || opcode === INSET_BLURRED_RECT) {
        const count = opcode === BLURRED_RECT ? 6 : 9;
        const values = take(count, 'shadow geometry');
        const shadowColor = color('shadow color');
        const [x, y, width, height, radius] = values;
        const sigma = opcode === BLURRED_RECT ? values[5]! : values[8]!;
        const mask = canvasKit.MaskFilter.MakeBlur(canvasKit.BlurStyle.Normal, sigma, true);
        setFill(shadowColor);
        paint.setMaskFilter(mask);
        skCanvas.drawRRect(canvasKit.RRectXY(rect(x!, y!, width!, height!), radius!, radius!), paint);
        paint.setMaskFilter(null);
        mask.delete();
        if (opcode === BLURRED_RECT) {
          const [hasOccluder] = take(1, 'shadow occluder flag');
          if (hasOccluder === 1) take(5, 'shadow occluder');
        }
        continue;
      }
      throw new CanvasKitReplayError('contract', `CanvasKit command buffer: unknown opcode ${opcode}`);
    }
    surface.flush();
  } finally {
    paint.delete();
  }
}

function registerResources(resource: CanvasKitSurface, packets: CanvasKitResource[]): void {
  const { canvasKit } = resource;
  for (const packet of packets) {
    if (packet.kind === 'font') {
      if (resource.fonts.has(packet.id)) continue;
      const bytes = packet.bytes.buffer.slice(
        packet.bytes.byteOffset,
        packet.bytes.byteOffset + packet.bytes.byteLength,
      ) as ArrayBuffer;
      const typeface = canvasKit.Typeface.MakeFreeTypeFaceFromData(bytes);
      if (!typeface) throw new CanvasKitReplayError('environment', `CanvasKit font ${packet.id} decode failed`);
      resource.fonts.set(packet.id, typeface);
      continue;
    }
    if (packet.kind === 'image') {
      if (resource.images.has(packet.id)) continue;
      if (packet.bytes.byteLength !== packet.width * packet.height * 4) {
        throw new CanvasKitReplayError('contract', `CanvasKit image ${packet.id} payload length is invalid`);
      }
      const alphaType = [canvasKit.AlphaType.Opaque, canvasKit.AlphaType.Unpremul, canvasKit.AlphaType.Premul][packet.alphaType];
      if (!alphaType) throw new CanvasKitReplayError('contract', `CanvasKit image ${packet.id} alpha type is invalid`);
      const image = canvasKit.MakeImage({
        width: packet.width,
        height: packet.height,
        colorType: canvasKit.ColorType.RGBA_8888,
        alphaType,
        colorSpace: canvasKit.ColorSpace.SRGB,
      }, packet.bytes, packet.width * 4);
      if (!image) throw new CanvasKitReplayError('environment', `CanvasKit image ${packet.id} decode failed`);
      resource.images.set(packet.id, image);
    }
  }
}

function readPath(
  canvasKit: CanvasKit,
  commands: Float32Array,
  cursor: { offset: number },
) {
  if (cursor.offset >= commands.length) throw new CanvasKitReplayError('contract', 'CanvasKit path is truncated');
  const count = commands[cursor.offset++]!;
  const builder = new canvasKit.PathBuilder();
  const take = (amount: number) => {
    if (cursor.offset + amount > commands.length) {
      builder.delete();
      throw new CanvasKitReplayError('contract', 'CanvasKit path is truncated');
    }
    const values = Array.from(commands.subarray(cursor.offset, cursor.offset + amount));
    cursor.offset += amount;
    return values;
  };
  for (let index = 0; index < count; index += 1) {
    const verb = take(1)[0];
    if (verb === 0) builder.moveTo(...take(2) as [number, number]);
    else if (verb === 1) builder.lineTo(...take(2) as [number, number]);
    else if (verb === 2) builder.quadTo(...take(4) as [number, number, number, number]);
    else if (verb === 3) builder.cubicTo(...take(6) as [number, number, number, number, number, number]);
    else if (verb === 4) builder.close();
    else {
      builder.delete();
      throw new CanvasKitReplayError('contract', `CanvasKit path verb ${verb} is invalid`);
    }
  }
  return builder.detachAndDelete();
}

function resize(canvas: HTMLCanvasElement): void {
  const previous = requireSurface(canvas);
  previous.surface.delete();
  const replacement = createSurface(canvas, previous.canvasKit);
  for (const [id, font] of previous.fonts) replacement.fonts.set(id, font);
  for (const [id, image] of previous.images) replacement.images.set(id, image);
  surfaces.set(canvas, replacement);
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
