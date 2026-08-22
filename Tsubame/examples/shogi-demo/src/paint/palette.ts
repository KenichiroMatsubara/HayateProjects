import type { Rgba } from '@torimi/tsubame-protocol-generated/recorder';

/**
 * 盤・駒の配色。painter は `Rgba`（0〜1 の4成分）を、要素側の style は CSS 文字列を
 * 使うので、同じ色が要る所は両方をここに並べて置く（対応が目で追える）。
 */

export const BOARD_WOOD: Rgba = [0.898, 0.792, 0.588, 1];
export const BOARD_EDGE: Rgba = [0.404, 0.302, 0.196, 1];
export const GRID_LINE: Rgba = [0.29, 0.216, 0.141, 1];

/**
 * 駒の地色。木地より明るい飴色。**先後で色を変えない** — 先後は駒の向きが表す
 * （紙の将棋と同じ）。色で誤魔化していたのは、文字を回せなかった頃の名残。
 */
export const PIECE_FACE: Rgba = [0.965, 0.898, 0.749, 1];
export const PIECE_EDGE: Rgba = [0.325, 0.239, 0.161, 1];
/** 駒の字。 */
export const PIECE_INK: Rgba = [0.102, 0.078, 0.063, 1];
/** 成駒の字は朱で書く（向きとは独立した情報なので色で分ける）。 */
export const PIECE_INK_PROMOTED: Rgba = [0.616, 0.145, 0.09, 1];

/** 選択中のマス。 */
export const SELECTED: Rgba = [0.114, 0.612, 0.851, 0.38];
/** 直前の指し手（移動元・移動先）。 */
export const LAST_MOVE: Rgba = [0.976, 0.792, 0.29, 0.42];
/** 合法手の候補マスに置く点。 */
export const TARGET_DOT: Rgba = [0.114, 0.435, 0.612, 0.55];
/** 駒を取れるマスは点ではなく輪で示す。 */
export const TARGET_RING: Rgba = [0.839, 0.294, 0.204, 0.75];
/** 王手を受けている玉。 */
export const CHECK_RING: Rgba = [0.839, 0.145, 0.145, 0.9];

/** 要素側 style 用（painter と対を成す CSS 文字列）。 */
export const CSS = {
  paper: '#f3ece0',
  panel: '#fffdf8ee',
  ink: '#1a1410',
  muted: '#6b6357',
  line: '#d9cfbc',
  accent: '#c2410c',
  boardEdge: '#674d32',
} as const;
