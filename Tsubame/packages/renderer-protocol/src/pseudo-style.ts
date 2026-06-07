import type { StylePatch } from './style.js';

export const PSEUDO_STYLE_KEYS = [':hover', ':active', ':focus'] as const;
export type PseudoStyleKey = (typeof PSEUDO_STYLE_KEYS)[number];

export type PseudoStylePatch = Partial<Record<PseudoStyleKey, StylePatch>>;

export const PSEUDO_STATE_CODE: Record<PseudoStyleKey, number> = {
  ':hover': 0,
  ':active': 1,
  ':focus': 2,
};

export function isPseudoStyleKey(key: string): key is PseudoStyleKey {
  return (PSEUDO_STYLE_KEYS as readonly string[]).includes(key);
}

/** Split a Hayate CSS `style` object into base props and pseudo-class blocks. */
export function splitHayateStyle(
  style: Record<string, unknown>,
): { base: StylePatch; pseudo: PseudoStylePatch } {
  const base: StylePatch = {};
  const pseudo: PseudoStylePatch = {};
  for (const [key, value] of Object.entries(style)) {
    if (isPseudoStyleKey(key)) {
      pseudo[key] = (value ?? {}) as StylePatch;
    } else {
      (base as Record<string, unknown>)[key] = value;
    }
  }
  return { base, pseudo };
}
