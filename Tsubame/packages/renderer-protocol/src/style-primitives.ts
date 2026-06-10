/**
 * Primitive value types referenced by the generated `HayateStyle`/`StylePatch`
 * (`./generated/style-types.ts`). Not mechanically derivable from
 * `style_tags.json`/`enums.json`, so kept hand-written.
 */
export type HayateDimension = number | `${number}px` | `${number}%` | `${number}fr` | 'auto';
