export type { ElementId, ElementKind } from './element.js';
export { asElementId } from './element.js';

export type {
  Display,
  FlexDirection,
  AlignItems,
  JustifyContent,
  FontStyle,
  TextDecoration,
  HayateDimension,
  HayateShadow,
  HayateStyle,
  StylePatch,
  HayateCssStyle,
} from './style.js';

export type {
  EventKind,
  InteractionEvent,
  EventHandler,
  Unsubscribe,
} from './event.js';

export type { IRenderer } from './renderer.js';

export type { ElementPropertyName, ElementPropertyOp } from './property.js';
export {
  ELEMENT_PROPERTY_NAMES,
  assertKnownElementProperty,
  isKnownElementProperty,
  coerceElementProperty,
} from './property.js';

export type { PseudoStyleKey, PseudoStylePatch } from './pseudo-style.js';
export {
  PSEUDO_STATE_CODE,
  PSEUDO_STATE_PRIORITY,
  PSEUDO_STYLE_KEYS,
  PSEUDO_STYLE_KEYS_BY_PRIORITY,
  isPseudoStyleKey,
  splitHayateStyle,
} from './pseudo-style.js';

export type { ViewportCondition } from './viewport-condition.js';

export { isTextLocal, carriesTextLocal } from './generated/style-channel.js';
