import {
  ELEMENT_PROPERTY_NAMES,
  type ElementPropertyName,
} from './generated/element-property.js';

const KNOWN_PROPERTIES = new Set<string>(ELEMENT_PROPERTY_NAMES);

export function isKnownElementProperty(name: string): name is ElementPropertyName {
  return KNOWN_PROPERTIES.has(name);
}

export function assertKnownElementProperty(
  name: string,
): asserts name is ElementPropertyName {
  if (!isKnownElementProperty(name)) {
    throw new Error(
      `Unknown element property "${name}". Only ${ELEMENT_PROPERTY_NAMES.join(', ')} are allowed (ADR-0071).`,
    );
  }
}

// prop-op の語彙・coercion・共有 dispatch は proto/spec (element_properties) から生成し、
// DOM と Canvas が単一のソースを共有する（ADR-0008）。
export {
  ELEMENT_PROPERTY_NAMES,
  coerceElementProperty,
  dispatchElementPropertyOp,
} from './generated/element-property.js';
export type {
  ElementPropertyName,
  ElementPropertyOp,
  ElementPropertyEffects,
} from './generated/element-property.js';
