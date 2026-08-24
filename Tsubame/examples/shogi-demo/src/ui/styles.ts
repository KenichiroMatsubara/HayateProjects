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

/** 罫線の太さ（論理 px）。枡どうしの隙間の幅で、盤の背景色がそこから覗く。 */
const GRID_LINE_PX = 1;
/** 外枠の太さ（論理 px）。 */
const BOARD_EDGE_PX = 8;

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

/**
 * 盤。**絵ではなく要素**で組む — 木地・罫線・外枠はすべてここの style が持つ。
 *
 * 罫線は線を引くのではなく、9×9 grid の `gap` から盤の背景（罫線色）を覗かせて作る。
 * Hayate CSS には辺ごとのボーダーが無く、枡ごとに枠を付けると内側が二重になるため。
 */
export const board: HayateCssStyle = {
  width: '100%',
  maxWidth: 520,
  aspectRatio: 1,
  flexShrink: 1,
  display: 'grid',
  gridTemplateColumns: ['1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr'],
  gridTemplateRows: ['1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr', '1fr'],
  gap: GRID_LINE_PX,
  backgroundColor: CSS.gridLine,
  borderWidth: BOARD_EDGE_PX,
  borderStyle: 'solid',
  borderColor: CSS.boardEdge,
  boxSizing: 'border-box',
};

/** 枡の状態。地色（＝要素の背景）だけで表せるものはここに集める。 */
export type CellTone = 'plain' | 'selected' | 'last-move';

const CELL_BACKGROUND: Readonly<Record<CellTone, string>> = {
  plain: CSS.wood,
  selected: CSS.woodSelected,
  'last-move': CSS.woodLastMove,
};

/**
 * 盤のマス。地色・タップ領域・ハイライトを担う。**駒と印だけ**が `draw` で載る
 * （style で表せないのはその 2 つだけ）。
 *
 * ハイライトが背景色なのは、重ね順を絵で作らずに済ませるため — 背景は必ず draw の
 * 下に来るので、ハイライトが駒を覆う描き順の事故が起こらない（ADR-0141: 背景 →
 * ボーダー → draw → 子）。
 */
const CELL_STYLE: Readonly<Record<CellTone, HayateCssStyle>> = {
  plain: { backgroundColor: CELL_BACKGROUND.plain, cursor: 'pointer' },
  selected: { backgroundColor: CELL_BACKGROUND.selected, cursor: 'pointer' },
  'last-move': { backgroundColor: CELL_BACKGROUND['last-move'], cursor: 'pointer' },
};

export function cell(tone: CellTone): HayateCssStyle {
  return CELL_STYLE[tone];
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

/** 駒台の枠を並べる行。枠そのものが駒を描くので、ここは並べるだけ。 */
export const handStage: HayateCssStyle = {
  height: HAND_SLOT_PX,
  flexGrow: 1,
  display: 'flex',
  flexDirection: 'row',
  gap: HAND_GAP_PX,
};

/**
 * 持ち駒 1 枠。駒の五角形だけが `draw`、座布団（選択中）と枚数は要素が持つ。
 * 枚数を右下に置くために `position: relative` の基準にもなる。
 */
export function handSlot(selected: boolean): HayateCssStyle {
  return {
    width: HAND_SLOT_PX,
    height: '100%',
    position: 'relative',
    borderRadius: Math.round(HAND_SLOT_PX * 0.16),
    backgroundColor: selected ? CSS.handSelected : 'transparent',
    cursor: 'pointer',
  };
}

/** 2 枚以上のときだけ出す枚数。駒の絵の上（＝子要素）に来る。 */
export const handCount: HayateCssStyle = {
  position: 'absolute',
  right: 1,
  bottom: 0,
  defaultFontSize: 11,
  fontWeight: 700,
  defaultColor: CSS.accent,
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
