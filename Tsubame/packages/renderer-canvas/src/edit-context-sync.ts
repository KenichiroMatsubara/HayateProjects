import type { RawHayate } from './hayate.js';

/** One EditContext composition format range (`textformatupdate.getTextFormats()`).
 * Offsets are UTF-16 code-unit indices into the EditContext text. */
export interface EditTextFormat {
  rangeStart: number;
  rangeEnd: number;
  underlineStyle?: string;
  underlineThickness?: string;
}

const byteEncoder = new TextEncoder();

/** UTF-16 code-unit offset → UTF-8 byte offset within `text`. EditContext speaks
 * UTF-16 offsets; the Hayate core edit model speaks UTF-8 byte offsets, so
 * composition clause ranges must be converted before crossing the wasm wire. */
function utf16ToByteOffset(text: string, utf16Offset: number): number {
  const clamped = Math.max(0, Math.min(utf16Offset, text.length));
  return byteEncoder.encode(text.slice(0, clamped)).length;
}

/** Convert EditContext composition `textformatupdate` formats into the flat
 * `[start, end, weight, …]` UTF-8 byte-offset triple stream the wasm core
 * consumes (ADR-0102, #336). `text` is the current preedit; `base` is the
 * composing segment's start offset in EditContext text (UTF-16). Formats outside
 * the preedit, collapsed ranges, and explicitly non-underlined ranges are
 * dropped. `weight` is `1` for a thick underline (the active clause) else `0`. */
export function compositionFormatsToWire(
  text: string,
  base: number,
  formats: readonly EditTextFormat[],
): Uint32Array {
  const out: number[] = [];
  for (const f of formats) {
    if (f.underlineStyle === 'None') continue;
    const start = utf16ToByteOffset(text, f.rangeStart - base);
    const end = utf16ToByteOffset(text, f.rangeEnd - base);
    if (start >= end) continue;
    out.push(start, end, f.underlineThickness === 'Thick' ? 1 : 0);
  }
  return Uint32Array.from(out);
}

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
