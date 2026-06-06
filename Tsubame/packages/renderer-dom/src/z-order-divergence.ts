import type { ElementId, HayateStyle } from '@tsubame/renderer-protocol';

/**
 * Styles that may create implicit browser stacking contexts and diverge from
 * Canvas/Hayate Z-Order semantics. Extend when new Element style properties
 * are added (e.g. `transform`).
 */
export const Z_ORDER_DIVERGENCE_PROPERTIES: ReadonlySet<keyof HayateStyle> = new Set([
  'opacity',
  // 'transform' — add when Element style gains transform
]);

const warned = new Set<string>();

function warningKey(elementId: ElementId, property: string): string {
  return `${elementId as number}:${property}`;
}

/**
 * Warn once per session when a divergent style property is applied in dev.
 * No-op in production; does not throw.
 */
export function warnZOrderDivergence(elementId: ElementId, property: string): void {
  if (process.env.NODE_ENV === 'production') return;
  if (!Z_ORDER_DIVERGENCE_PROPERTIES.has(property as keyof HayateStyle)) return;

  const key = warningKey(elementId, property);
  if (warned.has(key)) return;
  warned.add(key);

  console.warn(
    `[Tsubame DOM Renderer] Setting "${property}" on element ${elementId as number} may cause Z-Order divergence from Canvas/Hayate semantics. See Tsubame/docs/adr/0006-dom-z-order-rn-web-emulation.md`,
  );
}

/** @internal Reset dedupe state — for tests only */
export function resetZOrderDivergenceWarnings(): void {
  warned.clear();
}
