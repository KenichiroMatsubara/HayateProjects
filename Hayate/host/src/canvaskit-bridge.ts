import CanvasKitInit, {
  type CanvasKit,
  type Font,
  type Image,
  type Paint,
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

const CANVASKIT_TEXT_ANTIALIAS = true;
const CANVASKIT_FONT_SUBPIXEL_POSITIONING = true;

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
  readonly paint: Paint;
  readonly fonts: Map<number, Typeface>;
  readonly fontInstances: Map<string, Font>;
  readonly glyphScratch: Map<number, { glyphs: Uint16Array; positions: Float32Array }>;
  dashScratch: number[];
  readonly images: Map<number, Image>;
  readonly layers: Map<number, { surface: Surface; image?: Image }>;
  commandPayload?: Float32Array;
  layerUpdatesSinceComposite: number;
  pendingLayerTimeMs: number;
  performance: CanvasKitPerformanceMetrics;
}

export interface CanvasKitPerformanceSnapshot {
  readonly replayCount: number;
  readonly fullSceneReplayCount: number;
  readonly layerReplayCount: number;
  readonly compositeFrameCount: number;
  readonly compositeOnlyFrameCount: number;
  readonly commandPayloadBytes: number;
  readonly commandPayloadAllocationCount: number;
  readonly paintAllocationCount: number;
  readonly fontAllocationCount: number;
  readonly scratchAllocationCount: number;
  /** Temporary JS arrays created while decoding the command stream. Hot replay keeps this zero. */
  readonly commandDecodeAllocationCount: number;
  readonly frameTimeMs: readonly number[];
  readonly webgl: CanvasKitWebGlInfo;
}

interface CanvasKitWebGlInfo {
  readonly version: string;
  readonly renderer: string;
  readonly software: boolean;
}

type CanvasKitPerformanceMetrics = {
  -readonly [Key in keyof CanvasKitPerformanceSnapshot]: Key extends 'frameTimeMs'
    ? number[]
    : CanvasKitPerformanceSnapshot[Key];
};

interface CanvasKitBridge {
  replay(
    canvas: HTMLCanvasElement,
    commands: Float32Array,
    resources?: CanvasKitResource[],
    commandLength?: number,
  ): void;
  replayLayer(
    canvas: HTMLCanvasElement,
    layer: number,
    commands: Float32Array,
    resources?: CanvasKitResource[],
    commandLength?: number,
  ): void;
  compositeLayers(
    canvas: HTMLCanvasElement,
    placements: Float64Array,
    background: Float32Array,
    contentScale: number,
  ): void;
  resize(canvas: HTMLCanvasElement): void;
  detach(canvas: HTMLCanvasElement): void;
  performanceSnapshot(canvas: HTMLCanvasElement): CanvasKitPerformanceSnapshot;
  resetPerformance(canvas: HTMLCanvasElement): void;
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
  const paint = new canvasKit.Paint();
  paint.setAntiAlias?.(CANVASKIT_TEXT_ANTIALIAS);
  return {
    canvasKit,
    surface,
    paint,
    fonts: new Map(),
    fontInstances: new Map(),
    glyphScratch: new Map(),
    dashScratch: [],
    images: new Map(),
    layers: new Map(),
    layerUpdatesSinceComposite: 0,
    pendingLayerTimeMs: 0,
    performance: {
      replayCount: 0,
      fullSceneReplayCount: 0,
      layerReplayCount: 0,
      compositeFrameCount: 0,
      compositeOnlyFrameCount: 0,
      commandPayloadBytes: 0,
      commandPayloadAllocationCount: 0,
      paintAllocationCount: 1,
      fontAllocationCount: 0,
      scratchAllocationCount: 0,
      commandDecodeAllocationCount: 0,
      frameTimeMs: [],
      webgl: readWebGlInfo(canvas),
    },
  };
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
  commandLength = commands.length,
  targetSurface?: Surface,
): void {
  const resourceCache = requireSurface(canvas);
  if (!Number.isInteger(commandLength) || commandLength < 0 || commandLength > commands.length) {
    throw new CanvasKitReplayError('contract', 'CanvasKit command length is invalid');
  }
  const { canvasKit } = resourceCache;
  const surface = targetSurface ?? resourceCache.surface;
  const layerReplay = targetSurface !== undefined;
  const startedAt = monotonicNow();
  resourceCache.performance.replayCount += 1;
  if (layerReplay) {
    resourceCache.performance.layerReplayCount += 1;
  } else {
    resourceCache.performance.fullSceneReplayCount += commandLength > 5 ? 1 : 0;
  }
  resourceCache.performance.commandPayloadBytes += commandLength * Float32Array.BYTES_PER_ELEMENT;
  if (resourceCache.commandPayload !== commands) {
    resourceCache.commandPayload = commands;
    resourceCache.performance.commandPayloadAllocationCount += 1;
  }
  registerResources(resourceCache, resources);
  const skCanvas = surface.getCanvas();
  const { paint } = resourceCache;
  let offset = 0;

  const take = (count: number, command: string): number => {
    if (offset + count > commandLength) {
      throw new CanvasKitReplayError('contract', `CanvasKit command buffer: truncated ${command}`);
    }
    const start = offset;
    offset += count;
    return start;
  };

  const value = (start: number, index = 0): number => commands[start + index]!;
  const color = (command: string) => {
    const start = take(4, command);
    return canvasKit.Color4f(value(start), value(start, 1), value(start, 2), value(start, 3));
  };
  const rect = (x: number, y: number, width: number, height: number) =>
    canvasKit.LTRBRect(x, y, x + width, y + height);
  const setFill = (value: ReturnType<typeof canvasKit.Color4f>) => {
    paint.setStyle(canvasKit.PaintStyle.Fill);
    paint.setColor(value);
  };

  try {
    while (offset < commandLength) {
      const opcode = commands[offset++]!;
      if (opcode === CLEAR) {
        skCanvas.clear(color('clear'));
        continue;
      }
      if (opcode === FILL_RECT) {
        const boundsStart = take(4, 'fillRect bounds');
        const x = value(boundsStart);
        const y = value(boundsStart, 1);
        const width = value(boundsStart, 2);
        const height = value(boundsStart, 3);
        const fillColor = color('fillRect color');
        const cornerRadius = value(take(1, 'fillRect radius'));
        const bounds = rect(x, y, width, height);
        setFill(fillColor);
        if (cornerRadius > 0) {
          skCanvas.drawRRect(canvasKit.RRectXY(bounds, cornerRadius, cornerRadius), paint);
        } else {
          skCanvas.drawRect(bounds, paint);
        }
        continue;
      }
      if (opcode === FILL_ROUNDED_RING || opcode === DASHED_BORDER) {
        const ringStart = take(6, 'ring');
        const x = value(ringStart);
        const y = value(ringStart, 1);
        const width = value(ringStart, 2);
        const height = value(ringStart, 3);
        const radius = value(ringStart, 4);
        const borderWidth = value(ringStart, 5);
        const ringColor = color('ring color');
        const outer = canvasKit.RRectXY(rect(x, y, width, height), radius, radius);
        setFill(ringColor);
        if (opcode === FILL_ROUNDED_RING) {
          const inset = borderWidth;
          const inner = canvasKit.RRectXY(
            rect(x + inset, y + inset, Math.max(0, width - inset * 2), Math.max(0, height - inset * 2)),
            Math.max(0, radius - inset),
            Math.max(0, radius - inset),
          );
          skCanvas.drawDRRect(outer, inner, paint);
        } else {
          paint.setStyle(canvasKit.PaintStyle.Stroke);
          paint.setStrokeWidth(borderWidth);
          const dash = canvasKit.PathEffect.MakeDash([borderWidth * 3, borderWidth * 2], 0);
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
          const fillRule = value(take(1, 'fill path rule'));
          const path = readPath(canvasKit, commands, commandLength, { get offset() { return offset; }, set offset(value) { offset = value; } });
          path.setFillType(fillRule === 1 ? canvasKit.FillType.EvenOdd : canvasKit.FillType.Winding);
          setFill(pathColor);
          skCanvas.drawPath(path, paint);
          path.delete();
        } else {
          const styleStart = take(5, 'stroke style');
          const width = value(styleStart);
          const cap = value(styleStart, 1);
          const join = value(styleStart, 2);
          const miter = value(styleStart, 3);
          const dashCount = value(styleStart, 4);
          const dashStart = take(dashCount, 'stroke dash');
          const dashOffset = value(take(1, 'stroke dash offset'));
          const path = readPath(canvasKit, commands, commandLength, { get offset() { return offset; }, set offset(value) { offset = value; } });
          paint.setStyle(canvasKit.PaintStyle.Stroke);
          paint.setColor(pathColor);
          paint.setStrokeWidth(width);
          paint.setStrokeCap([canvasKit.StrokeCap.Butt, canvasKit.StrokeCap.Round, canvasKit.StrokeCap.Square][cap]!);
          paint.setStrokeJoin([canvasKit.StrokeJoin.Miter, canvasKit.StrokeJoin.Round, canvasKit.StrokeJoin.Bevel][join]!);
          paint.setStrokeMiter(miter);
          let dash: number[] | null = null;
          if (dashCount > 0) {
            if (resourceCache.dashScratch.length < dashCount) {
              resourceCache.dashScratch = new Array<number>(dashCount);
              resourceCache.performance.scratchAllocationCount += 1;
            } else {
              resourceCache.dashScratch.length = dashCount;
            }
            dash = resourceCache.dashScratch;
            for (let index = 0; index < dashCount; index += 1) {
              dash[index] = value(dashStart, index);
            }
          }
          const effect = dash ? canvasKit.PathEffect.MakeDash(dash, dashOffset) : null;
          paint.setPathEffect(effect);
          skCanvas.drawPath(path, paint);
          paint.setPathEffect(null);
          effect?.delete();
          path.delete();
        }
        continue;
      }
      if (opcode === DRAW_TEXT) {
        const headerStart = take(4, 'text header');
        const id = value(headerStart);
        const x = value(headerStart, 1);
        const y = value(headerStart, 2);
        const size = value(headerStart, 3);
        const textColor = color('text color');
        const skewStart = take(2, 'text skew synthesis');
        const hasSkew = value(skewStart);
        const skew = value(skewStart, 1);
        const emboldenStart = take(2, 'text embolden synthesis');
        const hasEmbolden = value(emboldenStart);
        const embolden = value(emboldenStart, 1);
        const coordinateCount = value(take(1, 'text variation coordinate count'));
        const coordinateStart = take(coordinateCount, 'text variation coordinates');
        let fontKey = `${id}:${size}:${hasSkew}:${skew}:${hasEmbolden}:${embolden}`;
        for (let index = 0; index < coordinateCount; index += 1) {
          fontKey += `:${value(coordinateStart, index)}`;
        }
        const glyphCount = value(take(1, 'glyph count'));
        const glyphStart = take(glyphCount * 3, 'glyphs');
        let realGlyphCount = 0;
        for (let index = 0; index < glyphCount; index += 1) {
          if (value(glyphStart, index * 3) !== 0) realGlyphCount += 1;
        }
        let scratch = resourceCache.glyphScratch.get(realGlyphCount);
        if (!scratch) {
          scratch = {
            glyphs: new Uint16Array(realGlyphCount),
            positions: new Float32Array(realGlyphCount * 2),
          };
          resourceCache.glyphScratch.set(realGlyphCount, scratch);
          resourceCache.performance.scratchAllocationCount += 2;
        }
        let outputIndex = 0;
        for (let index = 0; index < glyphCount; index += 1) {
          const entryStart = glyphStart + index * 3;
          const glyph = value(entryStart);
          if (glyph !== 0) {
            scratch.glyphs[outputIndex] = glyph;
            scratch.positions[outputIndex * 2] = value(entryStart, 1);
            scratch.positions[outputIndex * 2 + 1] = value(entryStart, 2);
            outputIndex += 1;
          }
        }
        const typeface = resourceCache.fonts.get(id);
        if (!typeface) throw new CanvasKitReplayError('contract', `CanvasKit font resource ${id} is unresolved`);
        let font = resourceCache.fontInstances.get(fontKey);
        if (!font) {
          font = new canvasKit.Font(typeface, size);
          font.setSubpixel(CANVASKIT_FONT_SUBPIXEL_POSITIONING);
          if (hasSkew) font.setSkewX(skew);
          if (hasEmbolden) font.setEmbolden(true);
          resourceCache.fontInstances.set(fontKey, font);
          resourceCache.performance.fontAllocationCount += 1;
        }
        setFill(textColor);
        if (realGlyphCount > 0) {
          skCanvas.drawGlyphs(scratch.glyphs, scratch.positions, x, y, font, paint);
        }
        const placeholderCount = value(take(1, 'missing glyph placeholder count'));
        for (let index = 0; index < placeholderCount; index += 1) {
          const placeholderStart = take(5, 'missing glyph placeholder');
          const px = value(placeholderStart);
          const py = value(placeholderStart, 1);
          const width = value(placeholderStart, 2);
          const height = value(placeholderStart, 3);
          const strokeWidth = value(placeholderStart, 4);
          paint.setStyle(canvasKit.PaintStyle.Stroke);
          paint.setStrokeWidth(strokeWidth);
          skCanvas.drawRect(rect(x + px, y + py, width, height), paint);
        }
        setFill(textColor);
        const decorationCount = value(take(1, 'text decoration count'));
        for (let index = 0; index < decorationCount; index += 1) {
          const decorationStart = take(4, 'text decoration');
          const x0 = value(decorationStart);
          const x1 = value(decorationStart, 1);
          const decorationY = value(decorationStart, 2);
          const thickness = value(decorationStart, 3);
          skCanvas.drawRect(
            canvasKit.LTRBRect(
              x + x0, y + decorationY - thickness * 0.5,
              x + x1, y + decorationY + thickness * 0.5,
            ),
            paint,
          );
        }
        continue;
      }
      if (opcode === DRAW_IMAGE) {
        const imageStart = take(5, 'image');
        const id = value(imageStart);
        const x = value(imageStart, 1);
        const y = value(imageStart, 2);
        const width = value(imageStart, 3);
        const height = value(imageStart, 4);
        const image = resourceCache.images.get(id);
        if (!image) throw new CanvasKitReplayError('contract', `CanvasKit image resource ${id} is unresolved`);
        skCanvas.drawImageRect(
          image,
          canvasKit.LTRBRect(0, 0, image.width(), image.height()),
          rect(x, y, width, height),
          paint,
        );
        continue;
      }
      if (opcode === PUSH_TRANSFORM) {
        const transformStart = take(6, 'transform');
        skCanvas.save();
        skCanvas.concat([
          value(transformStart), value(transformStart, 2), value(transformStart, 4),
          value(transformStart, 1), value(transformStart, 3), value(transformStart, 5),
          0, 0, 1,
        ]);
        continue;
      }
      if (opcode === POP_TRANSFORM || opcode === POP_CLIP) {
        skCanvas.restore();
        continue;
      }
      if (opcode === PUSH_CLIP_RECT) {
        const clipStart = take(8, 'clip rect');
        const x = value(clipStart);
        const y = value(clipStart, 1);
        const width = value(clipStart, 2);
        const height = value(clipStart, 3);
        const tl = value(clipStart, 4);
        const tr = value(clipStart, 5);
        const br = value(clipStart, 6);
        const bl = value(clipStart, 7);
        skCanvas.save();
        const bounds = rect(x, y, width, height);
        if (tl === tr && tr === br && br === bl) {
          skCanvas.clipRRect(canvasKit.RRectXY(bounds, tl, tl), canvasKit.ClipOp.Intersect, true);
        } else {
          const builder = new canvasKit.PathBuilder();
          const radius = Math.max(tl, tr, br, bl);
          builder.addRRect(canvasKit.RRectXY(bounds, radius, radius));
          const path = builder.detachAndDelete();
          skCanvas.clipPath(path, canvasKit.ClipOp.Intersect, true);
          path.delete();
        }
        continue;
      }
      if (opcode === PUSH_CLIP_PATH) {
        const path = readPath(canvasKit, commands, commandLength, { get offset() { return offset; }, set offset(value) { offset = value; } });
        skCanvas.save();
        skCanvas.clipPath(path, canvasKit.ClipOp.Intersect, true);
        path.delete();
        continue;
      }
      if (opcode === BLURRED_RECT || opcode === INSET_BLURRED_RECT) {
        const count = opcode === BLURRED_RECT ? 6 : 9;
        const shadowStart = take(count, 'shadow geometry');
        const shadowColor = color('shadow color');
        const x = value(shadowStart);
        const y = value(shadowStart, 1);
        const width = value(shadowStart, 2);
        const height = value(shadowStart, 3);
        const radius = value(shadowStart, 4);
        const sigma = value(shadowStart, opcode === BLURRED_RECT ? 5 : 8);
        const mask = canvasKit.MaskFilter.MakeBlur(canvasKit.BlurStyle.Normal, sigma, true);
        setFill(shadowColor);
        paint.setMaskFilter(mask);
        skCanvas.drawRRect(canvasKit.RRectXY(rect(x, y, width, height), radius, radius), paint);
        paint.setMaskFilter(null);
        mask.delete();
        if (opcode === BLURRED_RECT) {
          const hasOccluder = value(take(1, 'shadow occluder flag'));
          if (hasOccluder === 1) take(5, 'shadow occluder');
        }
        continue;
      }
      throw new CanvasKitReplayError('contract', `CanvasKit command buffer: unknown opcode ${opcode}`);
    }
    surface.flush();
  } catch (error) {
    // A selected CanvasKit renderer never restarts or falls back after replay/surface failure.
    // Remove the Host-owned state before releasing it so re-entrant or later calls cannot double free.
    surfaces.delete(canvas);
    releaseSurface(resourceCache);
    throw error;
  } finally {
    const elapsed = monotonicNow() - startedAt;
    if (layerReplay) {
      resourceCache.pendingLayerTimeMs += elapsed;
    } else {
      resourceCache.performance.frameTimeMs.push(elapsed);
    }
  }
}

const LAYER_PLACEMENT_SLOTS = 12;

function replayLayer(
  canvas: HTMLCanvasElement,
  layer: number,
  commands: Float32Array,
  resources: CanvasKitResource[] = [],
  commandLength = commands.length,
): void {
  const resource = requireSurface(canvas);
  try {
    let cached = resource.layers.get(layer);
    if (!cached) {
      const surface = resource.surface.makeSurface(resource.surface.imageInfo());
      if (!surface) throw new CanvasKitReplayError('environment', `CanvasKit layer surface ${layer} unavailable`);
      cached = { surface };
      resource.layers.set(layer, cached);
    }
    replay(canvas, commands, resources, commandLength, cached.surface);
    const image = cached.surface.makeImageSnapshot();
    cached.image?.delete();
    cached.image = image;
    resource.layerUpdatesSinceComposite += 1;
  } catch (error) {
    if (surfaces.get(canvas) === resource) {
      surfaces.delete(canvas);
      releaseSurface(resource);
    }
    throw error;
  }
}

function compositeLayers(
  canvas: HTMLCanvasElement,
  placements: Float64Array,
  background: Float32Array,
  contentScale: number,
): void {
  const resource = requireSurface(canvas);
  const startedAt = monotonicNow();
  try {
    if (placements.length % LAYER_PLACEMENT_SLOTS !== 0) {
      throw new CanvasKitReplayError('contract', 'CanvasKit layer placement payload is invalid');
    }
    if (background.length !== 4 || !Number.isFinite(contentScale) || contentScale <= 0) {
      throw new CanvasKitReplayError('contract', 'CanvasKit layer composite parameters are invalid');
    }
    const { canvasKit, surface, paint } = resource;
    const target = surface.getCanvas();
    target.clear(canvasKit.Color4f(background[0]!, background[1]!, background[2]!, background[3]!));
    // Replay leaves the shared Paint configured for its last draw op. CanvasKit image drawing
    // modulates snapshots by that Paint color/filter, so normalize it before layer composition;
    // otherwise a final translucent shadow/text color tints the entire cached layer (ADR-0136's
    // real-browser alpha class, expressed through CanvasKit's Paint state).
    paint.setStyle(canvasKit.PaintStyle.Fill);
    paint.setColor(canvasKit.Color4f(1, 1, 1, 1));
    paint.setMaskFilter(null);
    paint.setPathEffect(null);
    const retained = new Set<number>();
    for (let start = 0; start < placements.length; start += LAYER_PLACEMENT_SLOTS) {
      const layer = placements[start]!;
      const cached = resource.layers.get(layer);
      if (!cached?.image) {
        throw new CanvasKitReplayError('contract', `CanvasKit layer ${layer} is unresolved`);
      }
      retained.add(layer);
      target.save();
      if (placements[start + 7] === 1) {
        target.clipRect(
          canvasKit.LTRBRect(
            placements[start + 8]! * contentScale,
            placements[start + 9]! * contentScale,
            (placements[start + 8]! + placements[start + 10]!) * contentScale,
            (placements[start + 9]! + placements[start + 11]!) * contentScale,
          ),
          canvasKit.ClipOp.Intersect,
          true,
        );
      }
      target.concat([
        placements[start + 1]!, placements[start + 3]!, placements[start + 5]! * contentScale,
        placements[start + 2]!, placements[start + 4]!, placements[start + 6]! * contentScale,
        0, 0, 1,
      ]);
      target.drawImage(cached.image, 0, 0, paint);
      target.restore();
    }
    surface.flush();
    for (const [layer, cached] of resource.layers) {
      if (retained.has(layer)) continue;
      cached.image?.delete();
      cached.surface.delete();
      resource.layers.delete(layer);
    }
    resource.performance.compositeFrameCount += 1;
    if (resource.layerUpdatesSinceComposite === 0) {
      resource.performance.compositeOnlyFrameCount += 1;
    }
    resource.layerUpdatesSinceComposite = 0;
    resource.performance.frameTimeMs.push(resource.pendingLayerTimeMs + monotonicNow() - startedAt);
    resource.pendingLayerTimeMs = 0;
  } catch (error) {
    surfaces.delete(canvas);
    releaseSurface(resource);
    throw error;
  }
}

function monotonicNow(): number {
  return globalThis.performance?.now() ?? Date.now();
}

function readWebGlInfo(canvas: HTMLCanvasElement): CanvasKitWebGlInfo {
  const gl = canvas.getContext?.('webgl2') ?? canvas.getContext?.('webgl');
  if (!gl) return { version: 'unavailable', renderer: 'unavailable', software: true };
  const extension = gl.getExtension('WEBGL_debug_renderer_info') as {
    UNMASKED_RENDERER_WEBGL: number;
  } | null;
  const version = String(gl.getParameter(gl.VERSION) ?? 'unknown');
  const renderer = String(gl.getParameter(extension?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER) ?? 'unknown');
  return {
    version,
    renderer,
    software: /swiftshader|llvmpipe|lavapipe|software/i.test(renderer),
  };
}

function performanceSnapshot(canvas: HTMLCanvasElement): CanvasKitPerformanceSnapshot {
  const metrics = requireSurface(canvas).performance;
  return { ...metrics, frameTimeMs: [...metrics.frameTimeMs], webgl: { ...metrics.webgl } };
}

function resetPerformance(canvas: HTMLCanvasElement): void {
  const resource = requireSurface(canvas);
  resource.performance = {
    replayCount: 0,
    fullSceneReplayCount: 0,
    layerReplayCount: 0,
    compositeFrameCount: 0,
    compositeOnlyFrameCount: 0,
    commandPayloadBytes: 0,
    commandPayloadAllocationCount: 0,
    paintAllocationCount: 0,
    fontAllocationCount: 0,
    scratchAllocationCount: 0,
    commandDecodeAllocationCount: 0,
    frameTimeMs: [],
    webgl: resource.performance.webgl,
  };
  resource.layerUpdatesSinceComposite = 0;
  resource.pendingLayerTimeMs = 0;
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
  commandLength: number,
  cursor: { offset: number },
) {
  if (cursor.offset >= commandLength) throw new CanvasKitReplayError('contract', 'CanvasKit path is truncated');
  const count = commands[cursor.offset++]!;
  const builder = new canvasKit.PathBuilder();
  const take = (amount: number) => {
    if (cursor.offset + amount > commandLength) {
      builder.delete();
      throw new CanvasKitReplayError('contract', 'CanvasKit path is truncated');
    }
    const start = cursor.offset;
    cursor.offset += amount;
    return start;
  };
  for (let index = 0; index < count; index += 1) {
    const verb = commands[take(1)]!;
    if (verb === 0) {
      const start = take(2);
      builder.moveTo(commands[start]!, commands[start + 1]!);
    } else if (verb === 1) {
      const start = take(2);
      builder.lineTo(commands[start]!, commands[start + 1]!);
    } else if (verb === 2) {
      const start = take(4);
      builder.quadTo(
        commands[start]!, commands[start + 1]!, commands[start + 2]!, commands[start + 3]!,
      );
    } else if (verb === 3) {
      const start = take(6);
      builder.cubicTo(
        commands[start]!, commands[start + 1]!, commands[start + 2]!,
        commands[start + 3]!, commands[start + 4]!, commands[start + 5]!,
      );
    }
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
  const performance = previous.performance;
  let replacement: CanvasKitSurface;
  try {
    replacement = createSurface(canvas, previous.canvasKit);
  } catch (error) {
    surfaces.delete(canvas);
    releaseSurface(previous);
    throw error;
  }
  for (const [id, font] of previous.fonts) replacement.fonts.set(id, font);
  for (const [key, font] of previous.fontInstances) replacement.fontInstances.set(key, font);
  for (const [count, scratch] of previous.glyphScratch) replacement.glyphScratch.set(count, scratch);
  replacement.dashScratch = previous.dashScratch;
  for (const [id, image] of previous.images) replacement.images.set(id, image);
  replacement.commandPayload = previous.commandPayload;
  previous.fonts.clear();
  previous.fontInstances.clear();
  previous.glyphScratch.clear();
  previous.dashScratch = [];
  previous.images.clear();
  releaseSurface(previous);
  replacement.performance = {
    ...performance,
    paintAllocationCount: performance.paintAllocationCount + 1,
    frameTimeMs: [...performance.frameTimeMs],
    webgl: replacement.performance.webgl,
  };
  surfaces.set(canvas, replacement);
}

function releaseSurface(resource: CanvasKitSurface): void {
  for (const layer of resource.layers.values()) {
    layer.image?.delete();
    layer.surface.delete();
  }
  for (const font of resource.fontInstances.values()) font.delete();
  for (const typeface of resource.fonts.values()) typeface.delete();
  for (const image of resource.images.values()) image.delete();
  resource.fontInstances.clear();
  resource.fonts.clear();
  resource.glyphScratch.clear();
  resource.dashScratch = [];
  resource.images.clear();
  resource.layers.clear();
  resource.paint.delete();
  resource.surface.delete();
}

function detach(canvas: HTMLCanvasElement): void {
  const resource = surfaces.get(canvas);
  if (!resource) return;
  surfaces.delete(canvas);
  releaseSurface(resource);
}

/** Releases the CanvasKit surface/cache owned for one WebHost canvas. Safe to call repeatedly. */
export function detachCanvasKitSurface(canvas: HTMLCanvasElement): void {
  detach(canvas);
}

function installBridge(): void {
  const target = globalThis as Record<string, unknown>;
  if (target[CANVASKIT_BRIDGE_KEY]) return;
  const bridge: CanvasKitBridge = {
    replay,
    replayLayer,
    compositeLayers,
    resize,
    detach,
    performanceSnapshot,
    resetPerformance,
  };
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
