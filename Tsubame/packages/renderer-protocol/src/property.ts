/** Closed element property vocabulary (ADR-0071). `aria-*` uses first-class APIs only. */
export const ELEMENT_PROPERTY_NAMES = [
  'value',
  'placeholder',
  'disabled',
  'src',
] as const;

export type ElementPropertyName = (typeof ELEMENT_PROPERTY_NAMES)[number];

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
