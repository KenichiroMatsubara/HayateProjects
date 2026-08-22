import { Path } from '@torimi/tsubame-protocol-generated/recorder';
import type { Rect } from './board-metrics.js';

/**
 * 駒の五角形。将棋の駒は上が尖った左右対称の五角形で、先後は**向き**で表す。
 *
 * ここは常に「頭が上」の形だけを返す。向きは {@link ../paint/piece.js} の
 * `paintPiece` が canvas の `rotate` で与える — 五角形と漢字が同じ変換の下に
 * 置かれる唯一の場所がそこなので、形の側が向きを知る必要はない。
 */

/** マスに対する駒の大きさ（マスいっぱいには描かない）。 */
const PIECE_SCALE = 0.86;
/** 駒幅に対する肩（尖り始める高さ）の位置。 */
const SHOULDER_RATIO = 0.26;
/** 駒幅に対する頭（上辺）の幅。 */
const HEAD_WIDTH_RATIO = 0.42;
/** 駒幅に対する足（下辺）の幅。 */
const FOOT_WIDTH_RATIO = 0.9;

/** `cell` の中央に収まる、頭が上を向いた駒の五角形を作る。 */
export function pieceShape(cell: Rect): Path {
  const size = Math.min(cell.width, cell.height) * PIECE_SCALE;
  const width = size * 0.88;
  const height = size;
  const cx = cell.x + cell.width / 2;
  const cy = cell.y + cell.height / 2;

  const halfHead = (width * HEAD_WIDTH_RATIO) / 2;
  const halfFoot = (width * FOOT_WIDTH_RATIO) / 2;
  const top = cy - height / 2;
  const shoulder = top + height * SHOULDER_RATIO;
  const bottom = cy + height / 2;

  const points: readonly (readonly [number, number])[] = [
    [cx, top],
    [cx + halfHead, shoulder],
    [cx + halfFoot, bottom],
    [cx - halfFoot, bottom],
    [cx - halfHead, shoulder],
  ];

  const path = new Path();
  points.forEach(([x, y], i) => {
    if (i === 0) path.moveTo(x, y);
    else path.lineTo(x, y);
  });
  path.close();
  return path;
}
