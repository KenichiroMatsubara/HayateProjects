import type { HayateColorRecord } from '@tsubame/protocol-generated/codec';

// Parsing/coercion lives in exactly one place — the generated codec (issue #235).
// Re-export so existing `./hayate.js` importers (and the package index) keep
// working without re-implementing parseColor/parseDimension here.
export {
  parseColor,
  parseDimension,
  finiteNumber,
  finiteInteger,
} from '@tsubame/protocol-generated/codec';
export type {
  HayateColorRecord,
  HayateDimensionRecord,
  HayateDimensionUnit,
} from '@tsubame/protocol-generated/codec';

/**
 * 実 Hayate WASM（`HayateElementRenderer`）が公開する WIT element-layer の
 * メソッドのうち、`CanvasRenderer` が呼び出すものだけを型付けした最小 interface。
 *
 * wasm-bindgen 生成クラスは構造的にこれを充足するため、`init.ts` では
 * 生成インスタンスをそのまま `RawHayate` として渡せる。スタイルは
 * `Float32Array`（style-packet TAG 形式）、ミューテーションは
 * `apply_mutations(ops, styles, texts)`（ADR-0052）、イベントは delivery
 * poll（ADR-0053: `[listener_id, kind, ...fields]`）でやり取りする。
 */
export interface RawHayate {
  element_create(id: number, kind: number): void;
  set_root(id: number): void;
  element_append_child(parent: number, child: number): void;
  element_insert_before(parent: number, child: number, before: number): void;
  element_remove(id: number): void;
  element_set_text(id: number, text: string): void;
  element_set_text_content(id: number, text: string): void;
  element_set_src(id: number, url: string): void;
  element_set_disabled(id: number, disabled: boolean): void;
  element_get_text(id: number): string;
  /** Returns the editable text content from the live tree. */
  element_get_text_content(id: number): string;
  /** Absolute layout bounds `[x, y, width, height]` from `layout_cache`. */
  element_get_bounds(id: number): Float32Array | number[];
  /** Element ids in `id` and its descendants (Hayate ElementTree is authoritative). */
  element_subtree_ids(id: number): Float64Array;
  element_set_style(id: number, packed: Float32Array): void;
  element_set_pseudo_style(id: number, state: number, packed: Float32Array): void;
  apply_mutations(
    ops: Float64Array,
    styles: Float32Array,
    texts: string[],
  ): void;
  on_resize(width: number, height: number, scale: number): void;
  on_pointer_move(x: number, y: number): void;
  on_pointer_down(x: number, y: number): void;
  on_pointer_up(x: number, y: number): void;
  on_wheel(x: number, y: number, deltaX: number, deltaY: number): void;
  on_key_down(key: string, modifiers: number): void;
  on_text_input(id: number, text: string): void;
  on_composition_start(id: number, text: string): void;
  on_composition_update(id: number, text: string): void;
  on_composition_end(id: number, text: string): void;
  focused_element_id(): number;
  /** Cursor rect synced during the last render (ADR-0069). */
  ime_character_bounds(): number[];
  /** JSON-encoded AccessKit `TreeUpdate` (ADR-0041), or `null` before layout. */
  poll_accessibility(): string | null;
  render(timestampMs: number): void;
  poll_events(): unknown[];
  register_listener(element_id: number, event_kind: number): number;
  set_background_color(r: number, g: number, b: number): void;
  /** Resolved style after inheritance + pseudo-state (ADR-0067), or `null` if `id` is unknown. */
  element_effective_visual(id: number): HayateEffectiveVisual | null;
}

/** JS-friendly mirror of `hayate_core::Visual` after effective-style resolution (ADR-0067). */
export interface HayateEffectiveVisual {
  backgroundColor: HayateColorRecord | null;
  opacity: number;
  borderRadius: number;
  borderWidth: number;
  borderColor: HayateColorRecord | null;
  textColor: HayateColorRecord | null;
  fontSize: number | null;
  fontWeight: number | null;
  fontStyle: 'normal' | 'italic' | 'oblique' | null;
  textDecoration: 'none' | 'underline' | 'line-through' | null;
  zIndex: number;
  fontFamily: string | null;
}
