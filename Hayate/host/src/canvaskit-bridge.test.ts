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
    const paint = { setColor: vi.fn(), delete: vi.fn() };
    const canvasKit = {
      MakeWebGLCanvasSurface: vi.fn(() => ({ getCanvas: () => ({ clear, drawRect }), flush, delete: vi.fn() })),
      Paint: class { constructor() { return paint; } },
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
});
