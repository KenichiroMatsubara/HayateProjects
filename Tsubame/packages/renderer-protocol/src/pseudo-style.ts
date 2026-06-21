export type {
  PseudoStyleKey,
  PseudoStylePatch,
} from './generated/pseudo-state.js';
export {
  PSEUDO_STATE_CODE,
  PSEUDO_STATE_PRIORITY,
  PSEUDO_STYLE_KEYS,
  PSEUDO_STYLE_KEYS_BY_PRIORITY,
} from './generated/pseudo-state.js';

import {
  PSEUDO_STYLE_KEYS,
  type PseudoStyleKey,
  type PseudoStylePatch,
} from './generated/pseudo-state.js';
import type { StylePatch } from './style.js';

export function isPseudoStyleKey(key: string): key is PseudoStyleKey {
  return (PSEUDO_STYLE_KEYS as readonly string[]).includes(key);
}

/** Hayate CSS の `style` を base プロパティと擬似クラスブロックに分割する。 */
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
