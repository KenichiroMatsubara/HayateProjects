import type { HayateCssStyle } from '@torimi/tsubame-renderer-protocol';
import { CSS } from '../paint/palette.js';
import { HAND_GAP_PX, HAND_SLOT_PX } from '../paint/hand-painter.js';

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
 * 盤のマス。**当たり判定だけ**を担う透明な箱で、子を持たない
 * （`InteractionEvent` の座標は viewport 基準で要素ローカルに落とせないため、
 * 座標計算ではなく要素でマスを取る）。絵は駒も含めてすべて painter が描く。
 */
export const cell: HayateCssStyle = {
  backgroundColor: 'transparent',
  cursor: 'pointer',
};

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

/**
 * 駒台の描画面。持ち駒の絵はこの一枚が `draw` で描き、子は枠ぶんのタップ領域だけ。
 * 幅・間隔は painter と同じ定数（`HAND_SLOT_PX` / `HAND_GAP_PX`）から取るので、
 * 絵と当たり判定がずれない。
 */
export const handStage: HayateCssStyle = {
  height: HAND_SLOT_PX,
  flexGrow: 1,
  display: 'flex',
  flexDirection: 'row',
  gap: HAND_GAP_PX,
};

/** 持ち駒 1 枠のタップ領域（盤の `cell` と同じく透明で子を持たない）。 */
export const handSlot: HayateCssStyle = {
  width: HAND_SLOT_PX,
  height: '100%',
  backgroundColor: 'transparent',
  cursor: 'pointer',
};
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
