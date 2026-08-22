import { Paint, PaintingStyle, Path, type Rgba } from '@torimi/tsubame-protocol-generated/recorder';
import type {
  DrawCanvas,
  DrawPainter,
  DrawSize,
} from '@torimi/tsubame-renderer-protocol';
import { Color, Square, type ImmutablePosition } from 'tsshogi';
import * as C from './palette.js';
import { boardMetrics, squareRect, type BoardMetrics, type Rect } from './board-metrics.js';
import { paintPiece } from './piece.js';

/**
 * 盤の painter。木地・罫線・星・ハイライト・**駒（五角形と漢字の両方）** を 1 枚の
 * display list で描く。盤を走査するのは 1 回だけで、駒は
 * {@link paintPiece} に任せる（駒を描く場所はそこ 1 箇所）。
 *
 * 再描画は {@link BoardPainter.shouldRepaint} が**プリミティブだけ**を比べて決める。
 * 局面オブジェクトはその場更新されるので identity 比較は効かない — 版数で見る。
 */
export interface BoardPainterState {
  readonly position: ImmutablePosition;
  /** 局面の版数（`ShogiGame.revision`）。再描画判定の主キー。 */
  readonly revision: number;
  /** 選択中のマス index。未選択は -1。 */
  readonly selected: number;
  /** 合法手の候補マス index。`selected` から導出されるので再描画キーには入れない。 */
  readonly targets: readonly number[];
  readonly lastFrom: number;
  readonly lastTo: number;
  /** 王手を受けている玉のマス index。無ければ -1。 */
  readonly checkedKing: number;
  readonly flipped: boolean;
}

/** 罫線の太さ（マス一辺に対する比）。 */
const GRID_STROKE_RATIO = 0.012;
/** 星（`hoshi`）の半径（マス一辺に対する比）。 */
const HOSHI_RATIO = 0.055;
/** 合法手ドットの半径（マス一辺に対する比）。 */
const DOT_RATIO = 0.14;

export class BoardPainter implements DrawPainter {
  constructor(private readonly state: BoardPainterState) {}

  paint(canvas: DrawCanvas, size: DrawSize): void {
    const metrics = boardMetrics(size);
    paintBoard(canvas, metrics);
    this.paintHighlights(canvas, metrics);
    this.paintPieces(canvas, metrics);
  }

  shouldRepaint(oldPainter: DrawPainter): boolean {
    if (!(oldPainter instanceof BoardPainter)) return true;
    const a = oldPainter.state;
    const b = this.state;
    // targets と checkedKing は revision + selected から一意に決まるので比べない
    // （導出できない状態だけを dirty key に入れる）。
    return (
      a.revision !== b.revision ||
      a.selected !== b.selected ||
      a.lastFrom !== b.lastFrom ||
      a.lastTo !== b.lastTo ||
      a.flipped !== b.flipped
    );
  }

  private paintHighlights(canvas: DrawCanvas, metrics: BoardMetrics): void {
    const { selected, targets, lastFrom, lastTo, checkedKing, flipped, position } = this.state;

    for (const index of [lastFrom, lastTo]) {
      if (index >= 0) fillRect(canvas, squareRect(metrics, index, flipped), C.LAST_MOVE);
    }
    if (selected >= 0) fillRect(canvas, squareRect(metrics, selected, flipped), C.SELECTED);

    for (const index of targets) {
      const rect = squareRect(metrics, index, flipped);
      const occupied = position.board.at(Square.newByIndex(index)) !== null;
      if (occupied) {
        // 駒を取れるマスは輪で示す（点だと駒の下に隠れる）。
        strokeCircle(canvas, rect, metrics.cell * 0.4, metrics.cell * 0.05, C.TARGET_RING);
      } else {
        fillCircle(canvas, rect, metrics.cell * DOT_RATIO, C.TARGET_DOT);
      }
    }

    if (checkedKing >= 0) {
      const rect = squareRect(metrics, checkedKing, flipped);
      strokeCircle(canvas, rect, metrics.cell * 0.42, metrics.cell * 0.06, C.CHECK_RING);
    }
  }

  private paintPieces(canvas: DrawCanvas, metrics: BoardMetrics): void {
    const { position, flipped } = this.state;
    for (const square of position.board.listNonEmptySquares()) {
      const piece = position.board.at(square);
      if (piece === null) continue;
      // 盤を反転しているときは、手前に来る側が上向きになるよう向きも反転する。
      const upright = (piece.color === Color.BLACK) !== flipped;
      paintPiece(canvas, squareRect(metrics, square.index, flipped), piece, !upright);
    }
  }
}

/** 木地・外枠・罫線・星。局面に依らないので独立した関数にしておく。 */
function paintBoard(canvas: DrawCanvas, metrics: BoardMetrics): void {
  fillRect(canvas, metrics.board, C.BOARD_WOOD);

  const edge = new Paint();
  edge.color = C.BOARD_EDGE;
  edge.style = PaintingStyle.stroke;
  edge.strokeWidth = metrics.edge;
  const frame = new Path();
  // 枠は枡目の**内側**へ描き込む（枡目を内側へ寄せない）。stroke は線の中心が輪郭に
  // 乗るので、盤の縁から半分だけ内へ寄せると枠がちょうど盤の中に収まる。
  frame.addRect(
    metrics.board.x + metrics.edge / 2,
    metrics.board.y + metrics.edge / 2,
    metrics.board.width - metrics.edge,
    metrics.board.height - metrics.edge,
  );
  canvas.drawPath(frame, edge);

  const lines = new Path();
  for (let i = 1; i < 9; i += 1) {
    const offset = i * metrics.cell;
    lines.moveTo(metrics.grid.x + offset, metrics.grid.y);
    lines.lineTo(metrics.grid.x + offset, metrics.grid.y + metrics.grid.height);
    lines.moveTo(metrics.grid.x, metrics.grid.y + offset);
    lines.lineTo(metrics.grid.x + metrics.grid.width, metrics.grid.y + offset);
  }
  const grid = new Paint();
  grid.color = C.GRID_LINE;
  grid.style = PaintingStyle.stroke;
  grid.strokeWidth = Math.max(0.5, metrics.cell * GRID_STROKE_RATIO);
  canvas.drawPath(lines, grid);

  // 星は 3 と 6 の交点の 4 箇所。
  const hoshi = new Path();
  for (const column of [3, 6]) {
    for (const row of [3, 6]) {
      hoshi.addCircle(
        metrics.grid.x + column * metrics.cell,
        metrics.grid.y + row * metrics.cell,
        metrics.cell * HOSHI_RATIO,
      );
    }
  }
  const hoshiPaint = new Paint();
  hoshiPaint.color = C.GRID_LINE;
  hoshiPaint.style = PaintingStyle.fill;
  canvas.drawPath(hoshi, hoshiPaint);
}

function fillRect(canvas: DrawCanvas, rect: Rect, color: Rgba): void {
  const paint = new Paint();
  paint.color = color;
  paint.style = PaintingStyle.fill;
  const path = new Path();
  path.addRect(rect.x, rect.y, rect.width, rect.height);
  canvas.drawPath(path, paint);
}

function fillCircle(canvas: DrawCanvas, rect: Rect, radius: number, color: Rgba): void {
  const paint = new Paint();
  paint.color = color;
  paint.style = PaintingStyle.fill;
  const path = new Path();
  path.addCircle(rect.x + rect.width / 2, rect.y + rect.height / 2, radius);
  canvas.drawPath(path, paint);
}

function strokeCircle(
  canvas: DrawCanvas,
  rect: Rect,
  radius: number,
  width: number,
  color: Rgba,
): void {
  const paint = new Paint();
  paint.color = color;
  paint.style = PaintingStyle.stroke;
  paint.strokeWidth = width;
  const path = new Path();
  path.addCircle(rect.x + rect.width / 2, rect.y + rect.height / 2, radius);
  canvas.drawPath(path, paint);
}
