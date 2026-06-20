// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import type { RawHayate } from './hayate.js';
import {
  attachTextInput,
  canvasPixelRectToDomRect,
  compositionFormatsToWire,
  syncEditContext,
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
    tabIndex: -1,
    addEventListener: vi.fn(),
    getBoundingClientRect: () => rect as DOMRect,
  } as unknown as HTMLCanvasElement;
}

/** A minimal EditContext mock that records bounds calls. */
function stubEditContext(): EditContext {
  return {
    addEventListener: vi.fn(),
    updateControlBounds: vi.fn(),
    updateSelectionBounds: vi.fn(),
    selectionStart: 0,
  } as unknown as EditContext;
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

describe('syncEditContext bounds', () => {
  it('places the candidate window on a host-managed EditContext while editing', () => {
    const editContext = stubEditContext();
    const canvas = stubCanvas(
      100,
      50,
      { left: 0, top: 0, width: 100, height: 50 },
      editContext,
    );

    const raw = {
      ime_wants_keyboard: () => true,
      ime_character_bounds: () => [10, 5, 4, 12],
    } as unknown as RawHayate;

    syncEditContext(canvas, raw);

    expect(editContext.updateControlBounds).toHaveBeenCalledOnce();
    expect(editContext.updateSelectionBounds).toHaveBeenCalledOnce();
    const rect = (editContext.updateControlBounds as ReturnType<typeof vi.fn>).mock
      .calls[0]![0] as DOMRect;
    expect(rect.x).toBe(10);
    expect(rect.y).toBe(5);
    expect(rect.width).toBe(4);
    expect(rect.height).toBe(12);
  });
});

// The mobile bug (#392): tapping any non-editable content raised the soft
// keyboard. The keyboard is raised by attaching our EditContext, so it must
// attach only while core reports a focused text-input (`ime_wants_keyboard`).
describe('syncEditContext keyboard gating (#392)', () => {
  function setup() {
    const editContext = stubEditContext();
    const canvas = stubCanvas(100, 50, { left: 0, top: 0, width: 100, height: 50 });
    let wants = false;
    const raw = {
      focused_element_id: () => 0,
      has_selection: () => false,
      ime_wants_keyboard: () => wants,
      ime_character_bounds: () => [0, 0, 4, 12],
    } as unknown as RawHayate;
    attachTextInput(canvas, raw, () => editContext);
    return { canvas, raw, editContext, setWants: (v: boolean) => (wants = v) };
  }

  it('does not attach the EditContext when no text-input is focused', () => {
    const { canvas, raw, setWants } = setup();
    setWants(false);
    syncEditContext(canvas, raw);
    // Not attached → no soft keyboard on a plain tap.
    expect(canvas.editContext == null).toBe(true);
  });

  it('attaches the EditContext when a text-input is focused', () => {
    const { canvas, raw, editContext, setWants } = setup();
    setWants(true);
    syncEditContext(canvas, raw);
    expect(canvas.editContext).toBe(editContext);
  });

  it('detaches the EditContext when focus leaves the text-input', () => {
    const { canvas, raw, editContext, setWants } = setup();
    setWants(true);
    syncEditContext(canvas, raw);
    expect(canvas.editContext).toBe(editContext);

    setWants(false);
    syncEditContext(canvas, raw);
    // Detached → soft keyboard dismisses.
    expect(canvas.editContext).toBeNull();
  });
});
