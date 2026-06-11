import type { StylePatch } from '@tsubame/renderer-protocol';
import { declarationsFromStylePatch } from './style-declarations.js';

export function applyStylePatch(el: HTMLElement, patch: StylePatch): void {
  const style = el.style as unknown as Record<string, string>;

  for (const decl of declarationsFromStylePatch(el, patch, { onUnknownKey: 'throw' })) {
    style[decl.cssName] = decl.value;
  }
}
