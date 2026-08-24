import { describe, expect, it } from 'vitest';
import { Canvas } from '@torimi/tsubame-protocol-generated/recorder';
import { Color, PieceType, Position } from 'tsshogi';
import { pieceLabel } from './piece.js';
import {
  HandSlotPainter,
  HAND_SLOT_PX,
  heldTypes,
  type HandSlotPainterState,
} from './hand-painter.js';
import { SpyCanvas } from './test-support.js';

const SIZE = { width: HAND_SLOT_PX, height: HAND_SLOT_PX };

/** 先手が歩 2・銀 1 を持っている局面。 */
const WITH_HAND = Position.newBySFEN(
  'lnsgkgsnl/1r5b1/pppppppp1/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b 2Ps 1',
)!;

function stateOf(overrides: Partial<HandSlotPainterState> = {}): HandSlotPainterState {
  return { type: PieceType.PAWN, flipped: false, ...overrides };
}

function record(state: HandSlotPainterState): readonly number[] {
  const canvas = new Canvas();
  new HandSlotPainter(state).paint(canvas, SIZE);
  return canvas.finish();
}

describe('HandSlotPainter', () => {
  it('駒台にも本物の駒（五角形と漢字）が出る', () => {
    const canvas = new SpyCanvas();
    new HandSlotPainter(stateOf({ type: PieceType.SILVER })).paint(canvas, SIZE);
    expect(canvas.calls.filter((c) => c.op === 'drawPath').length).toBeGreaterThanOrEqual(2);
    expect(canvas.texts().map((t) => t.text)).toContain(pieceLabel(PieceType.SILVER));
  });

  it('座布団も枚数も描かない — どちらも要素（style と text）が持つ', () => {
    // 選択の座布団は枠要素の背景、枚数は枠の子の text。painter が描くのは駒だけなので、
    // 「選んだ / 枚数が変わった」だけでは display list は 1 バイトも変わらない。
    const canvas = new SpyCanvas();
    new HandSlotPainter(stateOf()).paint(canvas, SIZE);
    expect(canvas.texts().map((t) => t.text)).toEqual([pieceLabel(PieceType.PAWN)]);
  });

  it('相手側の駒台では駒が反転する', () => {
    const upright = record(stateOf({ flipped: false }));
    const rotated = record(stateOf({ flipped: true }));
    expect(rotated).not.toEqual(upright);
    expect(rotated.length).toBeGreaterThan(upright.length); // rotate op のぶん
  });
});

describe('HandSlotPainter.shouldRepaint', () => {
  const base = new HandSlotPainter(stateOf());

  it('同じ駒・同じ向きなら再描画しない', () => {
    expect(new HandSlotPainter(stateOf()).shouldRepaint(base)).toBe(false);
  });

  it('駒種・向きが変われば再描画する', () => {
    expect(new HandSlotPainter(stateOf({ type: PieceType.GOLD })).shouldRepaint(base)).toBe(true);
    expect(new HandSlotPainter(stateOf({ flipped: true })).shouldRepaint(base)).toBe(true);
  });

  it('枚数が増えても駒の絵は変わらない — 版数を持ち込む必要がない', () => {
    // 以前は駒台 1 枚を painter が描いていたので、その場更新される hand の枚数を
    // 検出するために版数（revision）を再描画キーに混ぜていた。枠ごとに駒 1 枚だけを
    // 描く今は、painter の状態に枚数が入らないので、その仕掛けごと要らない。
    expect(new HandSlotPainter(stateOf()).shouldRepaint(base)).toBe(false);
  });

  it('別種の painter とは常に再描画する', () => {
    expect(new HandSlotPainter(stateOf()).shouldRepaint({ paint: () => {} })).toBe(true);
  });
});

describe('heldTypes', () => {
  it('持っている駒種だけを安定順で返す', () => {
    const held = heldTypes(WITH_HAND.hand(Color.BLACK));
    expect(held).toContain(PieceType.PAWN);
    expect(held).not.toContain(PieceType.ROOK);
    // 要素側の枠の並び順そのものなので、順序も安定させる。
    expect(heldTypes(WITH_HAND.hand(Color.BLACK))).toEqual(held);
  });
});
