import { Path } from '@torimi/tsubame-protocol-generated/recorder';
import type { Rect } from './board-metrics.js';

/**
 * 駒の五角形。将棋の駒は上が尖った左右対称の五角形で、先後は**向き**で表す。
 *
 * `DrawCanvas` は `rotate` を持つので五角形自体は 180° 回せる。一方 draw v1 には
 * **文字を描く命令が無い**ため、駒の漢字は要素側の `text` で重ねる（正立のまま色で
 * 先後を分ける）。draw にテキストが生えたら、漢字も同じ回転に乗せられる。
 */

/** マスに対する駒の大きさ（マスいっぱいには描かない）。 */
const PIECE_SCALE = 0.86;
/** 駒幅に対する肩（尖り始める高さ）の位置。 */
const SHOULDER_RATIO = 0.26;
/** 駒幅に対する頭（上辺）の幅。 */
const HEAD_WIDTH_RATIO = 0.42;
/** 駒幅に対する足（下辺）の幅。 */
const FOOT_WIDTH_RATIO = 0.9;

/**
 * `cell` に収まる駒の五角形を作る。`pointingUp` が偽なら上下反転した
 * （＝後手向きの）五角形を返す。
 */
export function pieceShape(cell: Rect, pointingUp: boolean): Path {
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

  // 先手向き（頭が上）の 5 点。後手はこれを中心で上下反転する。
  const points: readonly (readonly [number, number])[] = [
    [cx, top],
    [cx + halfHead, shoulder],
    [cx + halfFoot, bottom],
    [cx - halfFoot, bottom],
    [cx - halfHead, shoulder],
  ];

  const path = new Path();
  points.forEach(([x, y], i) => {
    const py = pointingUp ? y : 2 * cy - y;
    if (i === 0) path.moveTo(x, py);
    else path.lineTo(x, py);
  });
  path.close();
  return path;
}
