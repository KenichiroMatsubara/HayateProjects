import type { DrawCanvas, DrawPainter, DrawSize } from '@torimi/tsubame-renderer-protocol';
import { handPieceTypes, PieceType, type ImmutableHand } from 'tsshogi';
import { boxRect } from './board-metrics.js';
import { paintPiece } from './piece.js';

/**
 * 駒台の 1 枠を描く painter。盤の枡（{@link ./cell-painter.js}）と同じ考え方で、
 * **枠・座布団・枚数は要素**が持ち、painter は駒の五角形だけを描く。
 *
 * 駒台の駒は常に持ち主から見て正立する。盤を反転しているときは自分の駒台が手前に
 * 来るので、向きは「相手側の駒台か」で決まる。
 */

/**
 * 駒台 1 枠の一辺と枠どうしの間隔（論理 px）。**盤と違い駒台は伸縮しない**ので、
 * 大きさを比ではなく固定 px で置く。要素の style だけがこれを読む（painter は自分の
 * 箱の大きさを `DrawSize` で受け取るので、幾何を知る必要がない）。
 */
export const HAND_SLOT_PX = 34;
export const HAND_GAP_PX = 4;

export interface HandSlotPainterState {
  readonly type: PieceType;
  /** 相手側の駒台か（駒を 180° 回す）。 */
  readonly flipped: boolean;
}

export class HandSlotPainter implements DrawPainter {
  constructor(private readonly state: HandSlotPainterState) {}

  paint(canvas: DrawCanvas, size: DrawSize): void {
    const { type, flipped } = this.state;
    paintPiece(canvas, boxRect(size), { type }, flipped);
  }

  shouldRepaint(oldPainter: DrawPainter): boolean {
    if (!(oldPainter instanceof HandSlotPainter)) return true;
    const a = oldPainter.state;
    const b = this.state;
    return a.type !== b.type || a.flipped !== b.flipped;
  }
}

/** 1 枚以上持っている駒種（駒台に並ぶ順）。 */
export function heldTypes(hand: ImmutableHand): PieceType[] {
  return handPieceTypes.filter((type) => hand.count(type) > 0);
}
