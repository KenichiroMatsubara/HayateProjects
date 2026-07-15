import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  CANVASKIT_BRIDGE_KEY,
  prepareCanvasKitSurface,
  resetCanvasKitBridgeForTesting,
} from './canvaskit-bridge.js';

afterEach(resetCanvasKitBridgeForTesting);

describe('prepareCanvasKitSurface', () => {
  it('owns CanvasKit surface setup and replays clear/rect as one frame boundary', async () => {
    const clear = vi.fn();
    const drawRect = vi.fn();
    const flush = vi.fn();
    const paint = { setColor: vi.fn(), setStyle: vi.fn(), delete: vi.fn() };
    const canvasKit = {
      MakeWebGLCanvasSurface: vi.fn(() => ({ getCanvas: () => ({ clear, drawRect }), flush, delete: vi.fn() })),
      Paint: class { constructor() { return paint; } },
      PaintStyle: { Fill: 0 },
      Color4f: (...color: number[]) => color,
      LTRBRect: (...rect: number[]) => rect,
    };
    const initialize = vi.fn(async () => canvasKit) as never;
    const canvas = {} as HTMLCanvasElement;

    await prepareCanvasKitSurface(canvas, initialize);

    const bridge = (globalThis as Record<string, unknown>)[CANVASKIT_BRIDGE_KEY] as {
      replay(target: HTMLCanvasElement, commands: Float32Array): void;
    };
    bridge.replay(canvas, new Float32Array([0, 0.1, 0.2, 0.3, 1, 1, 2, 3, 4, 5, 1, 0, 0, 1, 0]));

    expect(initialize).toHaveBeenCalledOnce();
    expect(clear).toHaveBeenCalledOnce();
    expect(clear.mock.calls[0]![0]).toEqual([
      expect.closeTo(0.1),
      expect.closeTo(0.2),
      expect.closeTo(0.3),
      1,
    ]);
    expect(drawRect).toHaveBeenCalledWith([2, 3, 6, 8], paint);
    expect(flush).toHaveBeenCalledOnce();
    expect(paint.delete).toHaveBeenCalledOnce();
  });

  it('decodes an image resource once and reuses it across frame replays', async () => {
    const drawImageRect = vi.fn();
    const flush = vi.fn();
    const image = { width: () => 1, height: () => 1, delete: vi.fn() };
    const makeImage = vi.fn(() => image);
    const canvasKit = {
      MakeWebGLCanvasSurface: vi.fn(() => ({
        getCanvas: () => ({ drawImageRect }),
        flush,
        delete: vi.fn(),
      })),
      MakeImage: makeImage,
      Paint: class { setStyle() {} delete() {} },
      PaintStyle: { Fill: 0 },
      LTRBRect: (...rect: number[]) => rect,
      AlphaType: { Opaque: 0, Unpremul: 1, Premul: 2 },
      ColorType: { RGBA_8888: 0 },
      ColorSpace: { SRGB: {} },
    };
    const canvas = {} as HTMLCanvasElement;
    await prepareCanvasKitSurface(canvas, vi.fn(async () => canvasKit) as never);
    const bridge = (globalThis as Record<string, unknown>)[CANVASKIT_BRIDGE_KEY] as {
      replay(
        target: HTMLCanvasElement,
        commands: Float32Array,
        resources: Array<Record<string, unknown>>,
      ): void;
    };
    const commands = new Float32Array([7, 1, 2, 3, 4, 5]);
    const resource = {
      kind: 'image',
      id: 1,
      width: 1,
      height: 1,
      alphaType: 1,
      bytes: new Uint8Array([255, 0, 0, 255]),
    };

    bridge.replay(canvas, commands, [resource]);
    bridge.replay(canvas, commands, []);

    expect(makeImage).toHaveBeenCalledOnce();
    expect(drawImageRect).toHaveBeenCalledTimes(2);
    expect(flush).toHaveBeenCalledTimes(2);
  });

  it('replays transform, clip, and path commands in scene order', async () => {
    const calls: string[] = [];
    const path = { setFillType: vi.fn(), delete: vi.fn() };
    class PathBuilder {
      moveTo() { calls.push('move'); }
      lineTo() { calls.push('line'); }
      close() { calls.push('close'); return path; }
      detachAndDelete() { return path; }
    }
    const skCanvas = {
      save: () => calls.push('save'),
      concat: () => calls.push('concat'),
      clipRRect: () => calls.push('clip'),
      drawPath: () => calls.push('drawPath'),
      restore: () => calls.push('restore'),
    };
    const paint = { setStyle() {}, setColor() {}, delete() {} };
    const canvasKit = {
      MakeWebGLCanvasSurface: vi.fn(() => ({ getCanvas: () => skCanvas, flush: vi.fn(), delete: vi.fn() })),
      Paint: class { constructor() { return paint; } },
      PathBuilder,
      PaintStyle: { Fill: 0 },
      FillType: { Winding: 0, EvenOdd: 1 },
      ClipOp: { Intersect: 0 },
      Color4f: (...color: number[]) => color,
      LTRBRect: (...rect: number[]) => rect,
      RRectXY: (rect: number[], rx: number, ry: number) => ({ rect, rx, ry }),
    };
    const canvas = {} as HTMLCanvasElement;
    await prepareCanvasKitSurface(canvas, vi.fn(async () => canvasKit) as never);
    const bridge = (globalThis as Record<string, unknown>)[CANVASKIT_BRIDGE_KEY] as {
      replay(target: HTMLCanvasElement, commands: Float32Array): void;
    };

    bridge.replay(canvas, new Float32Array([
      8, 1, 0, 0, 1, 2, 3,
      10, 0, 0, 10, 10, 0, 0, 0, 0,
      4, 1, 0, 0, 1, 0, 3, 0, 0, 0, 1, 5, 5, 4,
      12,
      9,
    ]));

    expect(calls).toEqual([
      'save', 'concat',
      'save', 'clip',
      'move', 'line', 'close', 'drawPath',
      'restore', 'restore',
    ]);
  });

  it('replays text with antialiasing, subpixel positioning, synthesis, and a variation-specific font instance', async () => {
    const drawGlyphs = vi.fn();
    const paint = {
      setStyle: vi.fn(),
      setColor: vi.fn(),
      setAntiAlias: vi.fn(),
      delete: vi.fn(),
    };
    const fonts: Array<Record<string, ReturnType<typeof vi.fn>>> = [];
    class Font {
      setSubpixel = vi.fn();
      setSkewX = vi.fn();
      setEmbolden = vi.fn();
      delete = vi.fn();
      constructor() { fonts.push(this as never); }
    }
    const typeface = { delete: vi.fn() };
    const canvasKit = {
      MakeWebGLCanvasSurface: vi.fn(() => ({
        getCanvas: () => ({ drawGlyphs }),
        flush: vi.fn(),
        delete: vi.fn(),
      })),
      Typeface: { MakeFreeTypeFaceFromData: vi.fn(() => typeface) },
      Paint: class { constructor() { return paint; } },
      Font,
      PaintStyle: { Fill: 0 },
      Color4f: (...color: number[]) => color,
    };
    const canvas = {} as HTMLCanvasElement;
    await prepareCanvasKitSurface(canvas, vi.fn(async () => canvasKit) as never);
    const bridge = (globalThis as Record<string, unknown>)[CANVASKIT_BRIDGE_KEY] as {
      replay(target: HTMLCanvasElement, commands: Float32Array, resources: Array<Record<string, unknown>>): void;
    };

    const regular = new Float32Array([
      6, 1, 3, 4, 12,
      0.1, 0.2, 0.3, 1,
      0, 0,
      0, 0,
      0,
      1, 7, 1, 2,
      0,
      0,
    ]);
    const synthesizedVariable = new Float32Array([
      6, 1, 3, 4, 12,
      0.1, 0.2, 0.3, 1,
      1, 0.25,
      1, 18,
      2, 4096, -8192,
      1, 7, 1, 2,
      0,
      0,
    ]);
    bridge.replay(canvas, regular, [{ kind: 'font', id: 1, bytes: new Uint8Array([1, 2, 3, 4]) }]);
    bridge.replay(canvas, synthesizedVariable, []);
    bridge.replay(canvas, synthesizedVariable, []);

    expect(paint.setAntiAlias).toHaveBeenCalledWith(true);
    expect(fonts).toHaveLength(2);
    expect(fonts[1]!.setSubpixel).toHaveBeenCalledWith(true);
    expect(fonts[1]!.setSkewX).toHaveBeenCalledWith(0.25);
    expect(fonts[1]!.setEmbolden).toHaveBeenCalledWith(true);
    expect(drawGlyphs).toHaveBeenCalledTimes(3);
  });

  it('renders the shared missing-glyph placeholder and text decorations instead of glyph zero', async () => {
    const drawGlyphs = vi.fn();
    const drawRect = vi.fn();
    const paint = {
      setStyle: vi.fn(), setColor: vi.fn(), setAntiAlias: vi.fn(),
      setStrokeWidth: vi.fn(), delete: vi.fn(),
    };
    class Font {
      setSubpixel() {}
      delete() {}
    }
    const canvasKit = {
      MakeWebGLCanvasSurface: vi.fn(() => ({
        getCanvas: () => ({ drawGlyphs, drawRect }),
        flush: vi.fn(), delete: vi.fn(),
      })),
      Typeface: { MakeFreeTypeFaceFromData: vi.fn(() => ({})) },
      Paint: class { constructor() { return paint; } },
      Font,
      PaintStyle: { Fill: 0, Stroke: 1 },
      Color4f: (...color: number[]) => color,
      LTRBRect: (...bounds: number[]) => bounds,
    };
    const canvas = {} as HTMLCanvasElement;
    await prepareCanvasKitSurface(canvas, vi.fn(async () => canvasKit) as never);
    const bridge = (globalThis as Record<string, unknown>)[CANVASKIT_BRIDGE_KEY] as {
      replay(target: HTMLCanvasElement, commands: Float32Array, resources: Array<Record<string, unknown>>): void;
    };

    bridge.replay(canvas, new Float32Array([
      6, 1, 7, 9, 20,
      1, 0, 0, 1,
      0, 0, 0, 0, 0,
      2, 0, 2, 3, 7, 4, 5,
      1, 3.6, -9.8, 9.4, 12.4, 1.2,
      1, 1, 11, 5, 2,
    ]), [{ kind: 'font', id: 1, bytes: new Uint8Array([1]) }]);

    expect(drawGlyphs.mock.calls[0]![0]).toEqual(new Uint16Array([7]));
    expect(drawRect).toHaveBeenCalledWith(expect.arrayContaining([
      expect.closeTo(10.6), expect.closeTo(-0.8), expect.closeTo(20), expect.closeTo(11.6),
    ]), paint);
    expect(drawRect).toHaveBeenCalledWith([8, 13, 18, 15], paint);
  });

  it('classifies malformed payload as contract and decode failure as environment', async () => {
    const canvasKit = {
      MakeWebGLCanvasSurface: vi.fn(() => ({ getCanvas: () => ({}), flush: vi.fn(), delete: vi.fn() })),
      MakeImage: vi.fn(() => null),
      Paint: class {},
      AlphaType: { Opaque: {}, Unpremul: {}, Premul: {} },
      ColorType: { RGBA_8888: {} },
      ColorSpace: { SRGB: {} },
    };
    const canvas = {} as HTMLCanvasElement;
    await prepareCanvasKitSurface(canvas, vi.fn(async () => canvasKit) as never);
    const bridge = (globalThis as Record<string, unknown>)[CANVASKIT_BRIDGE_KEY] as {
      replay(target: HTMLCanvasElement, commands: Float32Array, resources: Array<Record<string, unknown>>): void;
    };
    const base = { kind: 'image', id: 1, width: 1, height: 1, alphaType: 1 };

    expect(() => bridge.replay(canvas, new Float32Array(), [
      { ...base, bytes: new Uint8Array([1]) },
    ])).toThrowError(expect.objectContaining({ category: 'contract' }));
    expect(() => bridge.replay(canvas, new Float32Array(), [
      { ...base, id: 2, bytes: new Uint8Array([1, 2, 3, 4]) },
    ])).toThrowError(expect.objectContaining({ category: 'environment' }));
  });
});
