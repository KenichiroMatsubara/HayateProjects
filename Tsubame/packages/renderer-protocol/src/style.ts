import type { PseudoStylePatch } from './pseudo-style.js';
import type { StylePatch } from './generated/style-types.js';

export type {
  HayateDimension,
  HayateGridLine,
  HayateGridPlacement,
  HayateShadow,
} from './style-primitives.js';
export type {
  Display,
  FlexDirection,
  FlexWrap,
  AlignItems,
  AlignSelf,
  AlignContent,
  JustifyContent,
  FontStyle,
  TextDecoration,
  BorderStyle,
  HayateStyle,
  StylePatch,
} from './generated/style-types.js';

/** Hayate の CSS 宣言。ベースパッチ + 任意の擬似クラスブロック。 */
export type HayateCssStyle = StylePatch & PseudoStylePatch;
