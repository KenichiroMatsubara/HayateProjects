import { Color, PieceType, type ImmutableHand } from 'tsshogi';
import { HandSlotPainter, heldTypes } from '../paint/hand-painter.js';
import * as S from './styles.js';

/**
 * 駒台。盤と同じ構図で、**枠・座布団・枚数は要素**、駒の五角形だけが `draw`。
 * 駒の絵そのものは盤と同じ {@link ../paint/piece.js} が描く。
 *
 * 打てるマスの判定は `rules/`（tsshogi）が持つので、ここは選択の面だけ。
 */
export interface HandProps {
  readonly color: Color;
  readonly hand: ImmutableHand;
  /** この駒台の手番か（相手の駒台は掴めない）。 */
  readonly active: boolean;
  readonly selected: PieceType | null;
  /** 盤の反転状態。相手側の駒台の駒は 180° 回る。 */
  readonly flipped: boolean;
  readonly onTapPiece: (pieceType: PieceType) => void;
}

export function Hand({ color, hand, active, selected, flipped, onTapPiece }: HandProps) {
  // 駒台の駒は持ち主から見て正立する。盤を反転すると自分の駒台が手前に来る。
  const upsideDown = (color === Color.BLACK) === flipped;

  return (
    <view style={S.handRow}>
      <text style={S.handSideLabel}>{color === Color.BLACK ? '☗先手' : '☖後手'}</text>
      <view style={S.handStage}>
        {heldTypes(hand).map((type) => {
          const count = hand.count(type);
          return (
            <view
              key={type}
              style={S.handSlot(selected === type)}
              user-select="none"
              onClick={active ? () => onTapPiece(type) : undefined}
              draw={new HandSlotPainter({ type, flipped: upsideDown })}
            >
              {count > 1 ? <text style={S.handCount}>{String(count)}</text> : null}
            </view>
          );
        })}
      </view>
    </view>
  );
}
