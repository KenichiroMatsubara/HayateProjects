import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import type { Palette } from '../theme';
import type { Priority } from '../todo-model';

/**
 * 共通イージング（ADR-0067 / Transition）。全インタラクティブ要素に同じ
 * 補間を載せ、hover / active / focus の状態切替を一瞬ではなく滑らかにする。
 * 補間対象は連続値（color / border / box-shadow / opacity / radius）のみ。
 */
export const EASE = { transitionDuration: 160, transitionTiming: 'ease-out' } as const;

/** アクセント色のグロー影。主要 CTA を POP に浮かせる（ADR-0095）。 */
export const glow = (color: string, strong = false): HayateCssStyle['boxShadow'] => [
  { offsetX: 0, offsetY: strong ? 8 : 5, blur: strong ? 22 : 16, spread: -4, color, inset: false },
];

/** 優先度→色。danger(高) / accent2(中) / blue(低) に対応する。 */
export function priorityTone(p: Palette, prio: Priority): string {
  if (prio === 3) return p.danger;
  if (prio === 2) return p.accent2;
  return p.blue;
}
