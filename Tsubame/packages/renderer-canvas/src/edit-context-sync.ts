import { MODIFIER } from '@tsubame/protocol-generated/protocol';
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

// The web ImeBridge host (ADR-0069). This module is the *only* place that may
// touch the platform `EditContext` — creating it, wiring its events, and
// attaching/detaching it from the canvas. Soft-keyboard visibility is decided by
// core (`ElementTree::drive_ime`, surfaced as `raw.ime_wants_keyboard()`); the
// host merely reflects it. Keeping every `EditContext` reference here (enforced
// by `ime-bridge-encapsulation.test.ts`) is what stops a per-platform gating
// divergence like #392 from recurring on the web.

/** The live EditContext per canvas. Held here, not on the canvas, so the
 * instance survives while detached (`canvas.editContext === null`). */
const editContexts = new WeakMap<HTMLCanvasElement, EditContext>();

/**
 * Create the canvas EditContext and wire its IME / keyboard events (ADR-0069).
 *
 * The EditContext is *not* attached at startup. Attaching it is what raises the
 * mobile soft keyboard, so attachment is deferred to {@link syncEditContext},
 * which attaches only while core reports a focused `text-input`
 * (`raw.ime_wants_keyboard()`). This is the fix for #392 — previously the
 * EditContext was attached permanently, so any tap (which focuses the canvas)
 * summoned the keyboard even on non-editable content.
 */
export function attachTextInput(
  canvas: HTMLCanvasElement,
  raw: RawHayate,
  // Injectable for tests; production uses the platform `EditContext`. When the
  // platform lacks EditContext (HTML mode, ADR-0016) and no factory is supplied,
  // IME wiring is skipped entirely.
  createEditContext?: () => EditContext,
): void {
  const make =
    createEditContext ??
    (typeof EditContext === 'undefined' ? null : () => new EditContext());
  if (make === null) return;

  canvas.tabIndex = 0;
  const editContext = make();
  editContexts.set(canvas, editContext);
  let composing = false;
  // The composing segment's start offset (UTF-16) and current preedit text,
  // tracked so `textformatupdate` clause ranges can be made preedit-relative and
  // converted to UTF-8 byte offsets before crossing the wire (ADR-0102, #336).
  let composeBase = 0;
  let composeText = '';

  editContext.addEventListener('compositionstart', () => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = true;
    composeBase = editContext.selectionStart;
    composeText = '';
    raw.on_composition_start(id, '');
  });

  editContext.addEventListener('textupdate', (e: TextUpdateEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    const text = e.text ?? '';
    if (composing) {
      composeBase = e.updateRangeStart;
      composeText = text;
      // Plain (unformatted) update first; the conversion underlines arrive in
      // the `textformatupdate` that follows and re-sends with clause ranges.
      raw.on_composition_update(id, text);
    } else {
      raw.on_text_input(id, text);
    }
  });

  editContext.addEventListener('textformatupdate', (e: TextFormatUpdateEvent) => {
    if (!composing) return;
    const id = raw.focused_element_id();
    if (id === 0) return;
    const formats = e.getTextFormats() as unknown as EditTextFormat[];
    const wire = compositionFormatsToWire(composeText, composeBase, formats);
    raw.on_composition_update_formatted(id, composeText, wire);
  });

  editContext.addEventListener('compositionend', (e: CompositionEndEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = false;
    composeText = '';
    raw.on_composition_end(id, e.data ?? '');
  });

  canvas.addEventListener('keydown', (e) => {
    const id = raw.focused_element_id();
    // Selection keyboard gestures (Ctrl/Cmd+A, Shift+Arrow — #267) act on the
    // document-wide selection, so dispatch them even with nothing focused (a
    // read-only Selection Region). Core consumes selection keys internally.
    if (id === 0 && !raw.has_selection()) return;
    if (composing && e.key !== 'Escape') {
      e.preventDefault();
      return;
    }

    let mods = 0;
    if (e.shiftKey) mods |= MODIFIER.SHIFT;
    if (e.ctrlKey) mods |= MODIFIER.CTRL;
    if (e.altKey) mods |= MODIFIER.ALT;
    if (e.metaKey) mods |= MODIFIER.META;
    raw.on_key_down(e.key, mods);

    const isPrintable = e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey;
    if (!isPrintable) {
      e.preventDefault();
    }
  });
}

/**
 * Reflect core's IME presentation onto the canvas EditContext each frame
 * (ADR-0069, #392).
 *
 * - `raw.ime_wants_keyboard()` false → detach the EditContext so the soft
 *   keyboard dismisses (and a plain tap never raises it).
 * - true → attach it (raising the keyboard) and aim the IME candidate window at
 *   the caret's character bounds.
 */
export function syncEditContext(canvas: HTMLCanvasElement, raw: RawHayate): void {
  const wants = raw.ime_wants_keyboard();
  const owned = editContexts.get(canvas);

  // For the EditContext we own (created in `attachTextInput`), attaching it is
  // what raises the mobile soft keyboard — so attach only while a `text-input`
  // is focused and detach otherwise (#392). A host-managed EditContext (embedded
  // renderers, tests) is left to its owner; we only place its candidate window.
  if (owned !== undefined) {
    if (wants) {
      if (canvas.editContext !== owned) canvas.editContext = owned;
    } else if (canvas.editContext === owned) {
      canvas.editContext = null;
    }
  }

  if (!wants) return;
  const editContext = canvas.editContext;
  if (editContext === undefined || editContext === null) return;

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
