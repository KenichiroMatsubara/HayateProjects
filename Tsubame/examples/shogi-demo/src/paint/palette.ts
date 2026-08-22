import type { Rgba } from '@torimi/tsubame-protocol-generated/recorder';

/**
 * 盤・駒の配色。painter は `Rgba`（0〜1 の4成分）を、要素側の style は CSS 文字列を
 * 使うので、同じ色が要る所は両方をここに並べて置く（対応が目で追える）。
 */

export const BOARD_WOOD: Rgba = [0.898, 0.792, 0.588, 1];
export const BOARD_EDGE: Rgba = [0.404, 0.302, 0.196, 1];
export const GRID_LINE: Rgba = [0.29, 0.216, 0.141, 1];

/** 先手（下側）の駒。木地より明るい飴色。 */
export const PIECE_BLACK: Rgba = [0.965, 0.898, 0.749, 1];
/** 後手（上側）の駒。回転で向きは分かるが、地色も一段濃くして遠目にも効かせる。 */
export const PIECE_WHITE: Rgba = [0.847, 0.741, 0.565, 1];
export const PIECE_EDGE: Rgba = [0.325, 0.239, 0.161, 1];

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
  /** 先手の駒文字。 */
  pieceInkBlack: '#1a1410',
  /** 後手の駒文字。回転しても先後が一目で分かるよう色でも差を付ける。 */
  pieceInkWhite: '#7a1f12',
} as const;
