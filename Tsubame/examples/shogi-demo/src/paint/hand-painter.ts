import { Paint, PaintingStyle, Path, TextStyle } from '@torimi/tsubame-protocol-generated/recorder';
import type { DrawCanvas, DrawPainter, DrawSize } from '@torimi/tsubame-renderer-protocol';
import { Color, handPieceTypes, PieceType, type ImmutableHand } from 'tsshogi';
import * as C from './palette.js';
import { paintPiece } from './piece.js';
import type { Rect } from './board-metrics.js';

/**
 * 駒台の painter。持ち駒を**盤と同じ駒**として描く（{@link paintPiece}）。
 *
 * 駒台の駒は常に持ち主から見て正立する。盤を反転しているときは自分の駒台が
 * 手前に来るので、`flipped` はここでも「相手側の駒台か」で決まる。
 */

/**
 * 駒台 1 枠の一辺と枠どうしの間隔（論理 px）。
 *
 * **盤と違い駒台は伸縮しない**ので、幾何を比ではなく固定 px で置く。painter が描く
 * 駒の位置と、その上に重ねるタップ領域（`styles.handSlot` / `handStage`）が同じ値を
 * 読むための単一の出所 — 片方だけ動かせばずれる、が起こらない。
 */
export const HAND_SLOT_PX = 34;
export const HAND_GAP_PX = 4;

/** 持ち駒の枚数バッジの大きさ（駒の一辺に対する比）。 */
const COUNT_RATIO = 0.34;

export interface HandPainterState {
  readonly color: Color;
  readonly hand: ImmutableHand;
  /**
   * 局面の版数（`ShogiGame.revision`）。再描画判定の主キー。
   *
   * **持ち駒の中身を比べても駄目**: tsshogi の hand は局面と同じオブジェクトが
   * その場で更新されるので、古い painter が握っている参照も新しい枚数を返す。
   * 「駒を取ったのに駒台が空のまま」はこれで起きる（盤が `revision` で駆動して
   * いるのと同じ理由）。
   */
  readonly revision: number;
  /** 選択中の持ち駒。無ければ null。 */
  readonly selected: PieceType | null;
  /** 盤の反転状態。相手側の駒台の駒は 180° 回す。 */
  readonly flipped: boolean;
}

/**
 * 駒台に並ぶ `slot` 番目の枠。要素側のタップ領域は同じ幅・同じ間隔の flex 行なので、
 * 両者は自動的に重なる。
 */
export function handSlotRect(size: DrawSize, slot: number): Rect {
  const height = Math.min(size.height, HAND_SLOT_PX);
  return {
    x: slot * (HAND_SLOT_PX + HAND_GAP_PX),
    y: (size.height - height) / 2,
    width: HAND_SLOT_PX,
    height,
  };
}

export class HandPainter implements DrawPainter {
  constructor(private readonly state: HandPainterState) {}

  paint(canvas: DrawCanvas, size: DrawSize): void {
    const { color, hand, selected, flipped } = this.state;
    // 相手の駒台は 180° 回す。盤の駒と同じ規則（持ち主から見て正立）。
    const upright = (color === Color.BLACK) !== flipped;

    heldTypes(hand).forEach((type, slot) => {
      const rect = handSlotRect(size, slot);
      if (selected === type) fillSelection(canvas, rect);
      paintPiece(canvas, rect, { type }, !upright);

      const count = hand.count(type);
      if (count > 1) paintCount(canvas, rect, count);
    });
  }

  shouldRepaint(oldPainter: DrawPainter): boolean {
    if (!(oldPainter instanceof HandPainter)) return true;
    const a = oldPainter.state;
    const b = this.state;
    return (
      a.revision !== b.revision ||
      a.color !== b.color ||
      a.selected !== b.selected ||
      a.flipped !== b.flipped
    );
  }
}

/** 1 枚以上持っている駒種（駒台に並ぶ順）。 */
export function heldTypes(hand: ImmutableHand): PieceType[] {
  return handPieceTypes.filter((type) => hand.count(type) > 0);
}

/** 選択中の持ち駒を囲む座布団。 */
function fillSelection(canvas: DrawCanvas, rect: Rect): void {
  const paint = new Paint();
  paint.style = PaintingStyle.fill;
  paint.color = C.SELECTED;
  const path = new Path();
  path.addRRect(rect.x, rect.y, rect.width, rect.height, rect.width * 0.16, rect.width * 0.16);
  canvas.drawPath(path, paint);
}

/** 2 枚以上のときだけ出す枚数バッジ（駒の右下）。回転させない。 */
function paintCount(canvas: DrawCanvas, rect: Rect, count: number): void {
  const size = Math.min(rect.width, rect.height) * COUNT_RATIO;
  const style = new TextStyle();
  style.fontSize = size;
  style.fontWeight = 700;

  const ink = new Paint();
  ink.style = PaintingStyle.fill;
  ink.color = C.PIECE_INK_PROMOTED;
  canvas.drawText(
    String(count),
    rect.x + rect.width - size * 0.72,
    rect.y + rect.height - size,
    ink,
    style,
  );
}
