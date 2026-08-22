import { Color, Square, type ImmutablePosition } from 'tsshogi';
import { BoardPainter, type BoardPainterState } from '../paint/board-painter.js';
import { indexAtScreenOrder } from '../paint/board-metrics.js';
import { pieceLabel } from './piece-label.js';
import * as S from './styles.js';

/**
 * 盤。絵（木地・罫線・ハイライト・駒の五角形）は `draw` painter が 1 枚で描き、
 * その上に 81 個の透明なセル要素を 9×9 grid で敷いて**当たり判定と駒文字**を担わせる。
 *
 * 描画順は background → border → draw → children（CONTEXT.md）なので、painter の
 * 五角形は子の `text` の下に来る — 重ね順の指定は要らない。
 */

/** マス幅に対する駒文字の大きさ。 */
const GLYPH_RATIO = 0.62;
/** 盤の想定表示幅（駒文字の px を決めるためだけに使う概算）。 */
const ASSUMED_BOARD_WIDTH = 360;

export interface BoardProps {
  readonly position: ImmutablePosition;
  readonly painterState: BoardPainterState;
  readonly onTapSquare: (square: Square) => void;
}

export function Board({ position, painterState, onTapSquare }: BoardProps) {
  const glyph = Math.round(((ASSUMED_BOARD_WIDTH / 9) * GLYPH_RATIO));
  const flipped = painterState.flipped;

  return (
    <view style={S.board} draw={new BoardPainter(painterState)}>
      {Array.from({ length: 81 }, (_, order) => {
        const index = indexAtScreenOrder(order, flipped);
        const square = Square.newByIndex(index);
        const piece = position.board.at(square);
        return (
          <view key={order} style={S.cell} user-select="none" onClick={() => onTapSquare(square)}>
            {piece !== null ? (
              <text style={S.pieceText(piece.color === Color.WHITE, glyph)}>
                {pieceLabel(piece.type)}
              </text>
            ) : null}
          </view>
        );
      })}
    </view>
  );
}
