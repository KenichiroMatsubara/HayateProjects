export type Display = 'flex' | 'grid' | 'block' | 'none';
export type FlexDirection = 'row' | 'column' | 'row-reverse' | 'column-reverse';
export type AlignItems = 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
export type JustifyContent =
  | 'flex-start'
  | 'flex-end'
  | 'center'
  | 'space-between'
  | 'space-around'
  | 'space-evenly';

export type FontWeight = 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
export type HayateDimension = number | `${number}px` | `${number}%` | `${number}fr` | 'auto';

/**
 * Style properties accepted by the Renderer Protocol.
 *
 * Canvas Renderer converts these properties to Hayate's style packet format.
 * DOM Renderer maps the same names to browser inline styles for direct DOM rendering.
 */
export interface HayateStyle {
  // Sizing
  width: HayateDimension;
  height: HayateDimension;
  minWidth: HayateDimension;
  minHeight: HayateDimension;
  maxWidth: HayateDimension;
  maxHeight: HayateDimension;

  // Layout
  display: Display;
  flexDirection: FlexDirection;
  alignItems: AlignItems;
  justifyContent: JustifyContent;
  gap: HayateDimension;
  flexGrow: number;
  padding: HayateDimension;
  paddingTop: HayateDimension;
  paddingRight: HayateDimension;
  paddingBottom: HayateDimension;
  paddingLeft: HayateDimension;
  margin: HayateDimension;
  marginTop: HayateDimension;
  marginRight: HayateDimension;
  marginBottom: HayateDimension;
  marginLeft: HayateDimension;

  // Visual
  color: string;
  backgroundColor: string;
  borderColor: string;
  borderRadius: number;
  borderWidth: number;
  opacity: number;
  zIndex: number;

  // Text
  fontSize: number;
  fontFamily: string;
  /**
   * DOM Renderer only for now. Hayate Core has no FontWeight StyleProp yet,
   * so Canvas Renderer rejects this instead of silently dropping it.
   */
  fontWeight: FontWeight;
}

/**
 * Patch semantics for `IRenderer.setStyle`.
 *
 * - Present properties overwrite the previous value.
 * - Missing properties leave the previous value unchanged.
 * - `null` resets the property when the target renderer supports reset.
 */
export type StylePatch = {
  [K in keyof HayateStyle]?: HayateStyle[K] | null;
};
