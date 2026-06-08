import type { RawHayate } from './hayate.js';

/** Map canvas pixel coordinates to a screen-space DOMRect. */
export function canvasPixelRectToDomRect(
  canvas: HTMLCanvasElement,
  x: number,
  y: number,
  width: number,
  height: number,
): DOMRect {
  const rect = canvas.getBoundingClientRect();
  const scaleX = canvas.width === 0 ? 1 : rect.width / canvas.width;
  const scaleY = canvas.height === 0 ? 1 : rect.height / canvas.height;
  return new DOMRect(
    rect.left + x * scaleX,
    rect.top + y * scaleY,
    width * scaleX,
    height * scaleY,
  );
}

/** Apply the focused TextInput cursor rect to EditContext (ADR-0069). */
export function syncEditContextBounds(canvas: HTMLCanvasElement, raw: RawHayate): void {
  const editContext = canvas.editContext;
  if (editContext === undefined || editContext === null) return;

  const focused = raw.focused_element_id();
  if (focused === 0) return;

  const bounds = raw.ime_character_bounds();
  if (bounds[2] === 0 && bounds[3] === 0) return;

  const dom = canvasPixelRectToDomRect(
    canvas,
    bounds[0]!,
    bounds[1]!,
    bounds[2]!,
    bounds[3]!,
  );
  editContext.updateControlBounds(dom);
  editContext.updateSelectionBounds(dom);
}
