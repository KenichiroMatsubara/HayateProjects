import type { ElementKind, UserSelect } from '@tsubame/renderer-protocol';
import { elementKindDefaultUserSelect } from '@tsubame/renderer-protocol';

/**
 * Resolve the CSS `user-select` value for an element (ADR-0108, supersedes
 * ADR-0097 decision 2; refines decision 5). DOM Mode uses the browser's native
 * selection, bounded by `user-select`.
 *
 * The semantic is the single source of parity between the DOM Renderer and
 * Hayate HTML Mode (`resolve_user_select` in Rust). The shared corpus in
 * `proto/spec/fixtures/user_select_parity.json` pins both sides against drift.
 *
 * Resolution order: explicit `user-select` → element-kind UA default →
 * (none/unselectable). Selectability is opt-out, mirroring CSS:
 *
 * - `text-input` always owns its editing selection, so it is `text` regardless
 *   of any explicit value or kind default.
 * - Otherwise the effective value is the explicit `user-select` if present,
 *   else the kind default (`view` / `text` / `scroll-view` = `text`,
 *   `image` / `button` = `none`).
 * - `text` maps to CSS `text`; `none` maps to CSS `none` and excludes the
 *   subtree.
 * - `contains` is selectable but establishes a containment boundary, so it maps
 *   to CSS `contain` (ADR-0108 decision 3, #400). Browsers that support
 *   `user-select: contain` confine native selection to the element; those that
 *   do not ignore the value and the core-side boundary clamp supplies the same
 *   semantics (semantics-only parity).
 */
export function resolveUserSelect(
  kind: ElementKind,
  explicit: UserSelect | undefined,
): 'text' | 'none' | 'contain' {
  if (kind === 'text-input') return 'text';
  const effective = explicit ?? elementKindDefaultUserSelect(kind);
  if (effective === 'none') return 'none';
  if (effective === 'contains') return 'contain';
  return 'text';
}
