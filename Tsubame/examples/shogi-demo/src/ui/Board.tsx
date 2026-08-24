import { Color, Square, type ImmutablePosition } from 'tsshogi';
import { CellPainter, isBlankCell, type CellMark, type CellPainterState } from '../paint/cell-painter.js';
import { indexAtScreenOrder, isHoshiCorner } from '../paint/board-metrics.js';
import * as S from './styles.js';

/**
 * 盤。**木地・罫線・外枠・ハイライトは要素**（{@link ./styles.js} の `board` / `cell`）が
 * 持ち、`draw` painter が描くのは要素の style では表せないもの — 駒の五角形・星・
 * 合法手の印 — だけ。
 *
 * 盤 1 枚を絵にしてしまうと、枡の位置を painter でも当たり判定でも計算する羽目になり、
 * どこか 1 マスが動くだけで 81 マスぶんを描き直すことになる。枡を要素にしておけば
 * レイアウトはレイアウトエンジンが、再描画は枡ごとの `shouldRepaint` が面倒を見る。
 */

/**
 * 盤の見た目を決める値。**版数（`revision`）を持たない** — 盤 1 枚を絵にしていた頃は
 * 「局面がその場更新されるので identity 比較が効かない」問題を版数で殴っていたが、
 * 枡ごとの painter は自分の枡の駒種と向きだけを比べるので、その仕掛けが要らない。
 */
export interface BoardViewState {
  readonly position: ImmutablePosition;
  /** 選択中のマス index。未選択は -1。 */
  readonly selected: number;
  /** 合法手の候補マス index。 */
  readonly targets: readonly number[];
  readonly lastFrom: number;
  readonly lastTo: number;
  /** 王手を受けている玉のマス index。無ければ -1。 */
  readonly checkedKing: number;
  readonly flipped: boolean;
}

export interface BoardProps {
  readonly state: BoardViewState;
  readonly onTapSquare: (square: Square) => void;
}

export function Board({ state, onTapSquare }: BoardProps) {
  const { position, selected, targets, lastFrom, lastTo, checkedKing, flipped } = state;
  const targetSet = new Set(targets);

  return (
    <view style={S.board}>
      {Array.from({ length: 81 }, (_, order) => {
        const index = indexAtScreenOrder(order, flipped);
        const square = Square.newByIndex(index);
        const piece = position.board.at(square);
        const cellState: CellPainterState = {
          piece,
          // 盤を反転しているときは、手前に来る側が上向きになるよう向きも反転する。
          flipped: piece !== null && (piece.color === Color.BLACK) === flipped,
          mark: markAt(index, targetSet, checkedKing, piece !== null),
          hoshi: isHoshiCorner(order),
        };
        return (
          <view
            key={order}
            style={S.cell(toneAt(index, selected, lastFrom, lastTo))}
            user-select="none"
            onClick={() => onTapSquare(square)}
            // 描くものが無い枡には display list を持たせない（null で確実に消える）。
            draw={isBlankCell(cellState) ? null : new CellPainter(cellState)}
          />
        );
      })}
    </view>
  );
}

/** 枡の地色。選択が直前手より優先（今触っている方を見せる）。 */
function toneAt(index: number, selected: number, lastFrom: number, lastTo: number): S.CellTone {
  if (index === selected) return 'selected';
  if (index === lastFrom || index === lastTo) return 'last-move';
  return 'plain';
}

/** 枡に置く印。候補マスの示し方は「駒があるか」で変わる（点は駒に隠れる）。 */
function markAt(
  index: number,
  targets: ReadonlySet<number>,
  checkedKing: number,
  occupied: boolean,
): CellMark {
  if (targets.has(index)) return occupied ? 'capture' : 'target';
  if (index === checkedKing) return 'check';
  return 'none';
}
