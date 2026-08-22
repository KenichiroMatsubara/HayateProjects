import { Color, PieceType, handPieceTypes, type ImmutableHand } from 'tsshogi';
import { pieceLabel } from './piece-label.js';
import * as S from './styles.js';

/**
 * 駒台。持っている駒種だけを並べ、タップで打つ駒を選ぶ。
 * 打てるマスの判定は `rules/` 側（tsshogi）が持つので、ここは選択の面だけ。
 */
export interface HandProps {
  readonly color: Color;
  readonly hand: ImmutableHand;
  /** この駒台の手番か（相手の駒台は掴めない）。 */
  readonly active: boolean;
  readonly selected: PieceType | null;
  readonly onTapPiece: (pieceType: PieceType) => void;
}

export function Hand({ color, hand, active, selected, onTapPiece }: HandProps) {
  const held = handPieceTypes.filter((type) => hand.count(type) > 0);
  return (
    <view style={S.handRow}>
      <text style={S.handSideLabel}>{color === Color.BLACK ? '☗先手' : '☖後手'}</text>
      {held.map((type) => (
        <view
          key={type}
          style={S.handPiece(selected === type)}
          user-select="none"
          onClick={active ? () => onTapPiece(type) : undefined}
        >
          <text style={S.handLabel}>{pieceLabel(type)}</text>
          {hand.count(type) > 1 ? <text style={S.handCount}>{String(hand.count(type))}</text> : null}
        </view>
      ))}
    </view>
  );
}
