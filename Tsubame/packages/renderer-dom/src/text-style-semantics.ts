import type { StylePatch } from '@tsubame/renderer-protocol';

/** Channel-1 text-local keys (ADR-0065 / ADR-0002). */
export const TEXT_LOCAL_PATCH_KEYS = new Set<keyof StylePatch>([
  'color',
  'fontFamily',
  'fontSize',
  'fontWeight',
  'fontStyle',
  'textDecoration',
]);

/** Elements that may receive channel-1 text styles as CSS (text + text-input leaf carriers). */
export function acceptsTextLocalStyles(el: HTMLElement): boolean {
  const tag = el.tagName;
  return tag === 'SPAN' || tag === 'INPUT';
}

export function shouldApplyTextLocalPatch(el: HTMLElement, patchKey: string): boolean {
  if (!TEXT_LOCAL_PATCH_KEYS.has(patchKey as keyof StylePatch)) return true;
  return acceptsTextLocalStyles(el);
}
