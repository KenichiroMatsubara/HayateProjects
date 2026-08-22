import { Color, PieceType, type ImmutableHand } from 'tsshogi';
import { HandPainter, heldTypes } from '../paint/hand-painter.js';
import * as S from './styles.js';

/**
 * 駒台。**駒の絵は `draw` painter が盤と同じ {@link ../paint/piece.js} で描き**、
 * その上に駒ごとのタップ領域を重ねる（盤の 81 セルと同じ構図）。
 *
 * 打てるマスの判定は `rules/`（tsshogi）が持つので、ここは選択の面だけ。
 */
export interface HandProps {
  readonly color: Color;
  readonly hand: ImmutableHand;
  /** 局面の版数。駒台の再描画キー（hand はその場更新されるので中身では判定できない）。 */
  readonly revision: number;
  /** この駒台の手番か（相手の駒台は掴めない）。 */
  readonly active: boolean;
  readonly selected: PieceType | null;
  /** 盤の反転状態。相手側の駒台の駒は 180° 回る。 */
  readonly flipped: boolean;
  readonly onTapPiece: (pieceType: PieceType) => void;
}

export function Hand({ color, hand, revision, active, selected, flipped, onTapPiece }: HandProps) {
  const held = heldTypes(hand);
  return (
    <view style={S.handRow}>
      <text style={S.handSideLabel}>{color === Color.BLACK ? '☗先手' : '☖後手'}</text>
      {/* 駒の絵はこの一枚に載る。子はタップ領域だけで、painter の駒の上に重なる。 */}
      <view style={S.handStage} draw={new HandPainter({ color, hand, revision, selected, flipped })}>
        {held.map((type) => (
          <view
            key={type}
            style={S.handSlot}
            user-select="none"
            onClick={active ? () => onTapPiece(type) : undefined}
          />
        ))}
      </view>
    </view>
  );
}
