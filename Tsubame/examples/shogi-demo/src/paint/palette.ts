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

/**
 * 0〜1 の `Rgba` を CSS の色文字列にする。同じ色を painter 用と style 用に二度書かない
 * ための変換で、`CSS` の中身はここを通して上の定数から導く。
 */
function css(color: Rgba): string {
  const byte = (v: number): string =>
    Math.round(Math.min(1, Math.max(0, v)) * 255)
      .toString(16)
      .padStart(2, '0');
  const [r, g, b, a] = color;
  return `#${byte(r)}${byte(g)}${byte(b)}${a >= 1 ? '' : byte(a)}`;
}

/**
 * 半透明の色を地の色へ焼き込んで不透明の 1 色にする。
 *
 * 盤の枡は「木地の上にハイライトを重ねた絵」ではなく**要素の背景 1 色**として塗る
 * （背景は 1 色しか持てない）。重ね順を絵で作らずに済むぶん、ハイライトは style の
 * 差し替えだけになり、駒の下に潜り込む順序も構造的に保たれる。
 */
function over(front: Rgba, back: Rgba): string {
  const a = front[3];
  return css([
    front[0] * a + back[0] * (1 - a),
    front[1] * a + back[1] * (1 - a),
    front[2] * a + back[2] * (1 - a),
    1,
  ]);
}

/**
 * 要素側 style 用（painter と対を成す CSS 文字列）。
 *
 * 盤の木地・罫線・外枠・枡のハイライトは**要素が持つ**（絵ではない）。draw painter が
 * 描くのは、要素の style では表せないもの — 駒の五角形・星・合法手の印だけ。
 */
export const CSS = {
  paper: '#f3ece0',
  panel: '#fffdf8ee',
  ink: '#1a1410',
  muted: '#6b6357',
  line: '#d9cfbc',
  accent: '#c2410c',
  boardEdge: css(BOARD_EDGE),
  /** 枡の地色。 */
  wood: css(BOARD_WOOD),
  /** 罫線。枡の隙間からこの色が覗く（盤要素の背景）。 */
  gridLine: css(GRID_LINE),
  /** 選択中の枡。 */
  woodSelected: over(SELECTED, BOARD_WOOD),
  /** 直前の指し手の枡。 */
  woodLastMove: over(LAST_MOVE, BOARD_WOOD),
  /** 駒台で選んでいる駒の座布団（駒台の地は木地ではないので半透明のまま重ねる）。 */
  handSelected: css(SELECTED),
} as const;
