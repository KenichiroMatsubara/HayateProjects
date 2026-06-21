// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @hayate/protocol-spec

import type { StylePatch } from '../style.js';

export const PSEUDO_STYLE_KEYS = [":focus",":hover",":active"] as const;
export type PseudoStyleKey = (typeof PSEUDO_STYLE_KEYS)[number];

export type PseudoStylePatch = Partial<Record<PseudoStyleKey, StylePatch>>;

export const PSEUDO_STATE_CODE: Record<PseudoStyleKey, number> = {
  ':focus': 2,
  ':hover': 0,
  ':active': 1,
};

/** カスケードの帯域順（昇順・後勝ち）。ワイヤーコードとは別物。 */
export const PSEUDO_STATE_PRIORITY: Record<PseudoStyleKey, number> = {
  ':focus': 0,
  ':hover': 1,
  ':active': 2,
};

/** 優先度帯域でソートした擬似キー（focus < hover < active）。 */
export const PSEUDO_STYLE_KEYS_BY_PRIORITY = [":focus",":hover",":active"] as const satisfies readonly PseudoStyleKey[];
