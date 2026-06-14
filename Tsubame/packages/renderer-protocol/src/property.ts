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

/**
 * Renderer-agnostic result of coercing a `setProperty(name, value)` call
 * (issue #235). The discriminant names the logical effect; DOM and Canvas
 * renderers apply the *same* coerced payload to their own medium, so the
 * type-coercion edge cases (null erasure, stringification, `Boolean()`) live
 * in exactly one place and can never drift between the two renderers.
 */
export type ElementPropertyOp =
  | { kind: 'text-content'; text: string }
  | { kind: 'placeholder'; text: string }
  | { kind: 'src'; text: string }
  | { kind: 'disabled'; disabled: boolean };

/** Coerce a known element property + raw value into its shared semantics. */
export function coerceElementProperty(
  name: ElementPropertyName,
  value: unknown,
): ElementPropertyOp {
  switch (name) {
    case 'value':
      // Editable text content: null/undefined clear it, everything else stringifies.
      return { kind: 'text-content', text: value == null ? '' : String(value) };
    case 'placeholder':
      return { kind: 'placeholder', text: typeof value === 'string' ? value : '' };
    case 'src':
      return { kind: 'src', text: typeof value === 'string' ? value : '' };
    case 'disabled':
      return { kind: 'disabled', disabled: Boolean(value) };
  }
}
