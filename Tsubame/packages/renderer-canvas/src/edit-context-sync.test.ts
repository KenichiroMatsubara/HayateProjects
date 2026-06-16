// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import type { RawHayate } from './hayate.js';
import {
  canvasPixelRectToDomRect,
  compositionFormatsToWire,
  syncEditContextBounds,
} from './edit-context-sync.js';

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

describe('compositionFormatsToWire', () => {
  it('converts UTF-16 clause ranges to UTF-8 byte triples relative to the base', () => {
    // Preedit "ぎゅうにゅう": 6 UTF-16 units, 18 UTF-8 bytes (3 bytes each). The
    // composing segment starts at EditContext offset 2 (two committed chars).
    const wire = compositionFormatsToWire('ぎゅうにゅう', 2, [
      { rangeStart: 2, rangeEnd: 5, underlineThickness: 'Thick' },
      { rangeStart: 5, rangeEnd: 8, underlineThickness: 'Thin' },
    ]);
    expect(Array.from(wire)).toEqual([0, 9, 1, 9, 18, 0]);
  });

  it('drops non-underlined and collapsed ranges', () => {
    const wire = compositionFormatsToWire('abc', 0, [
      { rangeStart: 0, rangeEnd: 1, underlineStyle: 'None', underlineThickness: 'Thick' },
      { rangeStart: 2, rangeEnd: 2, underlineThickness: 'Thin' },
      { rangeStart: 1, rangeEnd: 3, underlineThickness: 'Thin' },
    ]);
    expect(Array.from(wire)).toEqual([1, 3, 0]);
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
