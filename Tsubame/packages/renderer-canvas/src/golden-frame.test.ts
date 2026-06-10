import { afterEach, describe, expect, it, vi } from 'vitest';
import { createElement, renderTsubame, setProp } from '@tsubame/solid';
import { CanvasRenderer } from './canvas-renderer.js';
import { createNullHayate, type WasmHayateFixture } from './test-helpers/wasm-hayate.js';
import { manualScheduler } from './test-helpers/manual-scheduler.js';
import { captureGoldenFrame, type GoldenFrameImeBounds } from './golden-frame.js';

describe('golden frame cross-seam harness (ADR-0079)', () => {
  let fixture: WasmHayateFixture | null = null;
  let dispose: (() => void) | null = null;

  afterEach(() => {
    dispose?.();
    dispose = null;
    fixture?.dispose();
    fixture = null;
  });

  it('mounts a focused, typed text-input and captures a tree/style/layout/a11y/IME golden frame', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(fixture.raw, {
      ...sched,
      canvas: fixture.canvas,
    });

    const updateControlBounds = vi.fn();
    const updateSelectionBounds = vi.fn();
    (fixture.canvas as unknown as { editContext: EditContext }).editContext = {
      updateControlBounds,
      updateSelectionBounds,
    } as unknown as EditContext;
    // happy-dom canvases report a zero-size bounding rect; give it a 1:1
    // mapping to canvas pixels so the IME DOM rect below is non-trivial.
    fixture.canvas.getBoundingClientRect = () =>
      ({
        left: 0,
        top: 0,
        width: fixture!.canvas.width,
        height: fixture!.canvas.height,
      }) as DOMRect;

    let inputId = 0;
    dispose = renderTsubame(() => {
      const input = createElement('text-input');
      setProp(input, 'style', {
        width: '120px',
        height: '32px',
        backgroundColor: '#ffffff',
      });
      setProp(input, 'value', 'Hi');
      inputId = input.id;
      return input;
    }, renderer);

    // Shadow Tree reconcile -> Mutation Packet -> real WASM ElementTree.
    sched.tick(16);

    // Focus the text-input via a real pointer-down hit-test, then type.
    const [x, y, w, h] = Array.from(fixture.raw.element_get_bounds(inputId));
    fixture.raw.on_pointer_down(x! + w! / 2, y! + h! / 2);
    fixture.raw.on_text_input(inputId, '!');

    // Re-render -> ElementTree -> IME bounds sync via the EditContext stub.
    sched.tick(32);

    let imeBounds: GoldenFrameImeBounds | null = null;
    if (updateControlBounds.mock.calls.length > 0) {
      const dom = updateControlBounds.mock.calls[0]![0] as DOMRect;
      imeBounds = { x: dom.x, y: dom.y, width: dom.width, height: dom.height };
    }

    const frame = captureGoldenFrame(fixture.raw, 1, imeBounds);

    expect(frame.elements.find((el) => el.id === inputId)?.textContent).toBe('Hi!');
    expect(updateControlBounds).toHaveBeenCalledOnce();
    expect(updateSelectionBounds).toHaveBeenCalledOnce();
    expect(frame).toMatchSnapshot();
  });
});
