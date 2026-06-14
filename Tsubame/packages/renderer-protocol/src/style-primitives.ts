/**
 * Primitive value types referenced by the generated `HayateStyle`/`StylePatch`
 * (`./generated/style-types.ts`). Not mechanically derivable from
 * `style_tags.json`/`enums.json`, so kept hand-written.
 */
export type HayateDimension = number | `${number}px` | `${number}%` | `${number}fr` | 'auto';

/**
 * A single CSS box-shadow layer (ADR-0095). Offsets/blur/spread are CSS px;
 * `color` is any CSS color string; `inset` selects an inner shadow.
 */
export interface HayateShadow {
  offsetX: number;
  offsetY: number;
  blur: number;
  spread: number;
  color: string;
  inset: boolean;
}
