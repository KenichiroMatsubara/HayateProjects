import type { HayateEffectiveVisual, RawHayate } from './hayate.js';

/** A single element's structural, style and layout state (ADR-0079). */
export interface GoldenFrameElement {
  id: number;
  text: string;
  textContent: string;
  /** `[x, y, width, height]` from `layout_cache`. */
  bounds: number[];
  /** Resolved `Visual` (inheritance + pseudo, ADR-0067), or `null`. */
  visual: HayateEffectiveVisual | null;
}

/** A DOM-space rect (ADR-0069 IME character bounds), or `null` when unfocused. */
export interface GoldenFrameImeBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * JSON-serializable structured snapshot of document state, spanning the
 * Shadow Tree → Mutation Packet → `ElementTree` → IME/AccessKit seams
 * (ADR-0079). Compare with `toMatchSnapshot()` against a golden file.
 */
export interface GoldenFrame {
  elements: GoldenFrameElement[];
  accessibility: unknown;
  imeBounds: GoldenFrameImeBounds | null;
}

/**
 * Captures a golden frame for `rootId` and its descendants (in
 * `element_subtree_ids` order, which Hayate's `ElementTree` returns in
 * document order).
 */
export function captureGoldenFrame(
  raw: RawHayate,
  rootId: number,
  imeBounds: GoldenFrameImeBounds | null,
): GoldenFrame {
  const ids = Array.from(raw.element_subtree_ids(rootId), Number);

  const elements = ids.map((id) => ({
    id,
    text: raw.element_get_text(id),
    textContent: raw.element_get_text_content(id),
    bounds: Array.from(raw.element_get_bounds(id)),
    visual: raw.element_effective_visual(id),
  }));

  const accessibilityJson = raw.poll_accessibility();
  const accessibility =
    accessibilityJson === null ? null : (JSON.parse(accessibilityJson) as unknown);

  return { elements, accessibility, imeBounds };
}
