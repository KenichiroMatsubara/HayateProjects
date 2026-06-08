// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import type { RawHayate } from './hayate.js';
import { canvasPixelRectToDomRect, syncEditContextBounds } from './edit-context-sync.js';

function stubCanvas(
  width: number,
  height: number,
  rect: Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>,
  editContext?: EditContext,
): HTMLCanvasElement {
  return {
    width,
    height,
    editContext,
    getBoundingClientRect: () => rect as DOMRect,
  } as unknown as HTMLCanvasElement;
}

describe('canvasPixelRectToDomRect', () => {
  it('maps canvas pixels to CSS screen coordinates', () => {
    const canvas = stubCanvas(200, 100, {
      left: 10,
      top: 20,
      width: 400,
      height: 200,
    });

    const dom = canvasPixelRectToDomRect(canvas, 50, 25, 8, 16);
    expect(dom.x).toBe(110);
    expect(dom.y).toBe(70);
    expect(dom.width).toBe(16);
    expect(dom.height).toBe(32);
  });
});

describe('syncEditContextBounds', () => {
  it('updates EditContext when focused element has IME bounds', () => {
    const updateControlBounds = vi.fn();
    const updateSelectionBounds = vi.fn();
    const canvas = stubCanvas(
      100,
      50,
      { left: 0, top: 0, width: 100, height: 50 },
      {
        updateControlBounds,
        updateSelectionBounds,
      } as unknown as EditContext,
    );

    const raw = {
      focused_element_id: () => 42,
      ime_character_bounds: () => [10, 5, 4, 12],
    } as RawHayate;

    syncEditContextBounds(canvas, raw);

    expect(updateControlBounds).toHaveBeenCalledOnce();
    expect(updateSelectionBounds).toHaveBeenCalledOnce();
    const rect = updateControlBounds.mock.calls[0]![0] as DOMRect;
    expect(rect.x).toBe(10);
    expect(rect.y).toBe(5);
    expect(rect.width).toBe(4);
    expect(rect.height).toBe(12);
  });
});
