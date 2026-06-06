import type { StylePatch } from '@tsubame/renderer-protocol';
import {
  CATALOG_BY_KEY,
  applyDomExtras,
  formatDomCSSValue,
} from '@tsubame/hayate-css-catalog';

export function applyStylePatch(el: HTMLElement, patch: StylePatch): void {
  const style = el.style as unknown as Record<string, string>;

  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined) continue;

    const entry = CATALOG_BY_KEY[k as string];
    if (entry === undefined) {
      throw new Error(`DOMRenderer: unknown Hayate style property "${k}"`);
    }

    style[entry.cssName] = value === null ? '' : formatDomCSSValue(entry, value);
    if (value !== null) {
      applyDomExtras(style, entry, value);
    }
  }
}
