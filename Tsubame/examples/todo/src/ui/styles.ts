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

/**
 * タイトルのはみ出し方針（Issue #428）。タイトルボタンは flexGrow:1 の列に座り、
 * 優先度ラベルや並べ替え/削除コントロールと横幅を奪い合う。明示的な方針が無いと
 * 折り返して行高を押し広げてしまうため、行クランプ＋ellipsis で抑える。
 *
 * 値はいずれもプレースホルダの**名前付き定数**。最終的な方針/値の確定は手動の
 * フォローアップ（マジックナンバー禁止）。クランプは protocol の maxLines /
 * textOverflow / overflow で表現し、DOM（-webkit-line-clamp）と Canvas の双方が
 * 同じカタログ定義を解釈する＝レンダラー間で一致する。
 */
export const TITLE_MAX_LINES = 1;

/** はみ出したタイトルの末尾表現。clip（ばっさり）ではなく ellipsis（…）。 */
export const TITLE_TEXT_OVERFLOW = 'ellipsis' as const;

/** クランプを成立させるはみ出し制御。visible だと溢れて行を押し広げる。 */
export const TITLE_OVERFLOW = 'hidden' as const;

/** タイトルボタンの基本スタイル（はみ出し方針込み）。 */
export function titleStyle(p: Palette, done: boolean): HayateCssStyle {
  return {
    display: 'flex',
    alignItems: 'center',
    backgroundColor: 'transparent',
    defaultColor: done ? p.quiet : p.ink,
    defaultFontSize: 15,
    borderWidth: 0,
    borderStyle: 'solid',
    maxLines: TITLE_MAX_LINES,
    textOverflow: TITLE_TEXT_OVERFLOW,
    overflow: TITLE_OVERFLOW,
    ...EASE,
    ':hover': { defaultColor: p.accent },
  };
}
