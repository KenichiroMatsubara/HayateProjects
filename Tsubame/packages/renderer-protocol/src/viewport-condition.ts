/**
 * Viewport-based condition for a property-level style variant (ADR-0081).
 *
 * All axes are in px and AND-combined; `minWidth`/`minHeight` match
 * inclusively (`actual >= min*`) and `maxWidth`/`maxHeight` match
 * inclusively (`actual <= max*`), mirroring CSS `@media (min-width: ...)` /
 * `(max-width: ...)` etc.
 */
export interface ViewportCondition {
  readonly minWidth?: number;
  readonly maxWidth?: number;
  readonly minHeight?: number;
  readonly maxHeight?: number;
}
