import { Paint, PaintingStyle, Path, type Rgba } from '@torimi/tsubame-protocol-generated/recorder';
import type { DrawCanvas, DrawPainter, DrawSize } from '@torimi/tsubame-renderer-protocol';
import * as C from './palette.js';
import { boxRect } from './board-metrics.js';
import { paintPiece, type PieceFace } from './piece.js';

/**
 * 枡 1 つぶんの painter。
 *
 * 盤の絵のうち**要素の style で表せるものは要素が持つ**（木地・罫線・外枠・枡の
 * ハイライト）。ここが描くのは style では表せないものだけ — 駒の五角形、星、
 * 合法手の印。これが Flutter 系の `CustomPaint` の使い方で、盤 1 枚を絵にしてしまうと
 * レイアウトも当たり判定も自前で座標計算する羽目になる。
 *
 * painter は自分の箱（`DrawSize`）しか見ない。枡の位置はレイアウトが決めるので、
 * 「絵の座標」と「タップの座標」がずれる余地が構造的に無い。
 */

/** 星の半径（枡の一辺に対する比）。 */
const HOSHI_RATIO = 0.055;
/** 合法手ドットの半径（枡の一辺に対する比）。 */
const DOT_RATIO = 0.14;
/** 駒を取れる枡に描く輪の半径と太さ。 */
const CAPTURE_RING_RATIO = 0.4;
const CAPTURE_RING_WIDTH_RATIO = 0.05;
/** 王手を受けている玉の輪。 */
const CHECK_RING_RATIO = 0.42;
const CHECK_RING_WIDTH_RATIO = 0.06;

/**
 * 枡に置く印。**同時に 1 つしか出ない**ので列挙で持つ（重ねる印が要るなら型を変える）。
 * - `target`: 空きマスへ指せる
 * - `capture`: 駒を取れる（点だと駒に隠れるので輪で示す）
 * - `check`: 王手を受けている玉
 */
export type CellMark = 'none' | 'target' | 'capture' | 'check';

export interface CellPainterState {
  /** その枡の駒。無ければ `null`。 */
  readonly piece: PieceFace | null;
  /** 駒を 180° 回すか（相手の駒）。 */
  readonly flipped: boolean;
  readonly mark: CellMark;
  /** 左上の角に星を打つか。 */
  readonly hoshi: boolean;
}

/** 何も描くものが無い枡か（`draw` を付けない = display list を持たせない）。 */
export function isBlankCell(state: CellPainterState): boolean {
  return state.piece === null && state.mark === 'none' && !state.hoshi;
}

export class CellPainter implements DrawPainter {
  constructor(private readonly state: CellPainterState) {}

  paint(canvas: DrawCanvas, size: DrawSize): void {
    const { piece, flipped, mark, hoshi } = this.state;
    const rect = boxRect(size);
    const side = Math.min(size.width, size.height);

    // 星は枡の左上の角（＝枡と枡の交点）。箱の外へはみ出して描く。
    if (hoshi) fillCircle(canvas, 0, 0, side * HOSHI_RATIO, C.GRID_LINE);

    // 印は駒の下に敷く（駒を取れる枡の輪は駒の縁からはみ出して見える）。
    const cx = size.width / 2;
    const cy = size.height / 2;
    if (mark === 'target') {
      fillCircle(canvas, cx, cy, side * DOT_RATIO, C.TARGET_DOT);
    } else if (mark === 'capture') {
      strokeCircle(
        canvas,
        cx,
        cy,
        side * CAPTURE_RING_RATIO,
        side * CAPTURE_RING_WIDTH_RATIO,
        C.TARGET_RING,
      );
    } else if (mark === 'check') {
      strokeCircle(
        canvas,
        cx,
        cy,
        side * CHECK_RING_RATIO,
        side * CHECK_RING_WIDTH_RATIO,
        C.CHECK_RING,
      );
    }

    if (piece !== null) paintPiece(canvas, rect, piece, flipped);
  }

  /**
   * 再描画は**その枡の見た目を決める値だけ**で判定する。盤全体を 1 枚で描いていた頃は
   * どこか 1 マスが動けば 81 マスぶんを描き直していたが、枡ごとの painter なら動いた
   * 枡だけが dirty になる。
   */
  shouldRepaint(oldPainter: DrawPainter): boolean {
    if (!(oldPainter instanceof CellPainter)) return true;
    const a = oldPainter.state;
    const b = this.state;
    return (
      a.piece?.type !== b.piece?.type ||
      a.flipped !== b.flipped ||
      a.mark !== b.mark ||
      a.hoshi !== b.hoshi
    );
  }
}

function fillCircle(
  canvas: DrawCanvas,
  cx: number,
  cy: number,
  radius: number,
  color: Rgba,
): void {
  const paint = new Paint();
  paint.color = color;
  paint.style = PaintingStyle.fill;
  const path = new Path();
  path.addCircle(cx, cy, radius);
  canvas.drawPath(path, paint);
}

function strokeCircle(
  canvas: DrawCanvas,
  cx: number,
  cy: number,
  radius: number,
  width: number,
  color: Rgba,
): void {
  const paint = new Paint();
  paint.color = color;
  paint.style = PaintingStyle.stroke;
  paint.strokeWidth = width;
  const path = new Path();
  path.addCircle(cx, cy, radius);
  canvas.drawPath(path, paint);
}
