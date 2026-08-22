import type { HayateCssStyle } from '@torimi/tsubame-renderer-protocol';
import { CSS } from '../paint/palette.js';

/**
 * 画面の style 定数。house style（react-demo）に倣ってモジュール定数として並べる。
 *
 * safe-area は Hayate CSS に `env()` が無く、ネイティブホストが座標と viewport を
 * 補正済みで渡してくる（react-demo README）ので、固定の余白で受ける。
 */

const FONT = 'Noto Sans JP, Hiragino Sans, Yu Gothic, system-ui, sans-serif';

export const shell: HayateCssStyle = {
  width: '100%',
  height: '100%',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 6,
  paddingTop: 14,
  paddingBottom: 14,
  paddingLeft: 10,
  paddingRight: 10,
  backgroundColor: CSS.paper,
  defaultColor: CSS.ink,
  defaultFontFamily: FONT,
  overflow: 'hidden',
};

/** 盤。`aspectRatio: 1` で正方形を保ち、9×9 の grid が枡目と 1 対 1 に対応する。 */
export const board: HayateCssStyle = {
  width: '100%',
  maxWidth: 520,
  aspectRatio: 1,
  flexShrink: 1,
  display: 'grid',
  gridTemplateColumns: ['1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr'],
  gridTemplateRows: ['1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr'],
};

/**
 * 盤のマス。当たり判定だけを担い、絵は painter が描く
 * （`InteractionEvent` の座標は viewport 基準で要素ローカルに落とせないため、
 * 座標計算ではなく要素でマスを取る）。
 */
export const cell: HayateCssStyle = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  backgroundColor: 'transparent',
  cursor: 'pointer',
};

/** マスの駒文字。draw v1 に文字命令が無いので要素側で重ねる。 */
export function pieceText(gote: boolean, size: number): HayateCssStyle {
  return {
    defaultColor: gote ? CSS.pieceInkWhite : CSS.pieceInkBlack,
    defaultFontSize: size,
    fontWeight: 700,
  };
}

export const handRow: HayateCssStyle = {
  width: '100%',
  maxWidth: 520,
  minHeight: 42,
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  gap: 4,
  paddingLeft: 8,
  paddingRight: 8,
  paddingTop: 4,
  paddingBottom: 4,
  backgroundColor: CSS.panel,
  borderRadius: 10,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: CSS.line,
};

export function handPiece(selected: boolean): HayateCssStyle {
  return {
    display: 'flex',
    flexDirection: 'row',
    alignItems: 'center',
    gap: 1,
    paddingLeft: 7,
    paddingRight: 7,
    paddingTop: 4,
    paddingBottom: 4,
    borderRadius: 7,
    borderWidth: 1,
    borderStyle: 'solid',
    borderColor: selected ? CSS.accent : CSS.line,
    backgroundColor: selected ? '#fbe4d5' : '#ffffff',
    cursor: 'pointer',
  };
}

export const handLabel: HayateCssStyle = { defaultFontSize: 17, fontWeight: 700 };
export const handCount: HayateCssStyle = { defaultFontSize: 11, defaultColor: CSS.muted };
export const handSideLabel: HayateCssStyle = {
  defaultFontSize: 11,
  defaultColor: CSS.muted,
  paddingRight: 4,
};

export const statusBar: HayateCssStyle = {
  width: '100%',
  maxWidth: 520,
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  justifyContent: 'space-between',
  paddingLeft: 4,
  paddingRight: 4,
};

export const statusMain: HayateCssStyle = { defaultFontSize: 14, fontWeight: 700 };
export const statusSub: HayateCssStyle = { defaultFontSize: 11, defaultColor: CSS.muted };

export const controls: HayateCssStyle = {
  width: '100%',
  maxWidth: 520,
  display: 'flex',
  flexDirection: 'row',
  gap: 6,
  justifyContent: 'center',
};

export function controlButton(primary: boolean): HayateCssStyle {
  return {
    height: 34,
    paddingLeft: 12,
    paddingRight: 12,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    borderRadius: 9,
    borderWidth: 1,
    borderStyle: 'solid',
    borderColor: primary ? CSS.accent : CSS.line,
    backgroundColor: primary ? CSS.accent : '#ffffff',
    defaultColor: primary ? '#ffffff' : CSS.ink,
    defaultFontSize: 12,
    fontWeight: 650,
    cursor: 'pointer',
    ':active': { backgroundColor: primary ? '#9a330a' : '#f1e9db' },
  };
}

/** 成り選択。盤の上に全面のタップ受けを敷き、その上に問いを置く。 */
export const promptScrim: HayateCssStyle = {
  position: 'absolute',
  top: 0,
  left: 0,
  width: '100%',
  height: '100%',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  backgroundColor: '#1a141066',
  zIndex: 50,
};

export const promptCard: HayateCssStyle = {
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 10,
  paddingTop: 16,
  paddingBottom: 16,
  paddingLeft: 20,
  paddingRight: 20,
  backgroundColor: '#fffdf8',
  borderRadius: 14,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: CSS.line,
};

export const promptTitle: HayateCssStyle = { defaultFontSize: 15, fontWeight: 700 };
export const promptRow: HayateCssStyle = { display: 'flex', flexDirection: 'row', gap: 8 };
