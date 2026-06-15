import type { ElementKind } from '@tsubame/renderer-protocol';

/**
 * Map a Selection Region boundary to the browser `user-select` value
 * (ADR-0097 decision 5: DOM Mode uses native selection, bounded by
 * `user-select`).
 *
 * The semantic is the single source of parity between the DOM Renderer and
 * Hayate HTML Mode (`resolve_user_select` in Rust). The shared corpus in
 * `proto/spec/fixtures/user_select_parity.json` pins both sides against drift.
 *
 * - `text-input` is always selectable regardless of any Selection Region
 *   boundary (editing requires it).
 * - Otherwise an element is selectable only inside a `selectable` subtree;
 *   the default is `none`.
 */
export function resolveUserSelect(
  kind: ElementKind,
  selectable: boolean | undefined,
): 'text' | 'none' {
  if (kind === 'text-input') return 'text';
  return selectable === true ? 'text' : 'none';
}
