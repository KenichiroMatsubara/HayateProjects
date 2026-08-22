import { Square } from 'tsshogi';
import { BoardPainter, type BoardPainterState } from '../paint/board-painter.js';
import { indexAtScreenOrder } from '../paint/board-metrics.js';
import * as S from './styles.js';

/**
 * 盤。**絵はすべて `draw` painter が 1 枚で描く**（木地・罫線・ハイライト・駒）。
 * その上に 81 個の透明なセル要素を 9×9 grid で敷くが、これは**当たり判定だけ**を担う。
 *
 * 要素が残るのは `InteractionEvent` の座標が viewport 基準で、要素ローカルへ落とせない
 * から（座標からマスを逆算する術がない）。逆に言えば、要素が担うのはそれだけで、
 * 子は持たない。
 */

export interface BoardProps {
  readonly painterState: BoardPainterState;
  readonly onTapSquare: (square: Square) => void;
}

export function Board({ painterState, onTapSquare }: BoardProps) {
  const flipped = painterState.flipped;

  return (
    <view style={S.board} draw={new BoardPainter(painterState)}>
      {Array.from({ length: 81 }, (_, order) => {
        const square = Square.newByIndex(indexAtScreenOrder(order, flipped));
        return (
          <view key={order} style={S.cell} user-select="none" onClick={() => onTapSquare(square)} />
        );
      })}
    </view>
  );
}
