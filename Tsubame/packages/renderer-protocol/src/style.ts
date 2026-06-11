import type { PseudoStylePatch } from './pseudo-style.js';
import type { StylePatch } from './generated/style-types.js';

export type { HayateDimension } from './style-primitives.js';
export type {
  Display,
  FlexDirection,
  AlignItems,
  AlignSelf,
  AlignContent,
  JustifyContent,
  FontStyle,
  TextDecoration,
  HayateStyle,
  StylePatch,
} from './generated/style-types.js';

/** Hayate CSS declaration: base patch plus optional pseudo-class blocks. */
export type HayateCssStyle = StylePatch & PseudoStylePatch;
