import { describe, expect, it } from 'vitest';
import { Canvas } from '@torimi/tsubame-protocol-generated/recorder';
import { Color, PieceType, Position } from 'tsshogi';
import { pieceLabel } from './piece.js';
import {
  HandPainter,
  HAND_GAP_PX,
  HAND_SLOT_PX,
  handSlotRect,
  heldTypes,
  type HandPainterState,
} from './hand-painter.js';
import { SpyCanvas } from './test-support.js';

const SIZE = { width: 320, height: HAND_SLOT_PX };

/** 先手が歩 2・銀 1 を持っている局面。 */
const WITH_HAND = Position.newBySFEN(
  'lnsgkgsnl/1r5b1/pppppppp1/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b 2Ps 1',
)!;

function stateOf(overrides: Partial<HandPainterState> = {}): HandPainterState {
  return {
    color: Color.BLACK,
    hand: WITH_HAND.hand(Color.BLACK),
    revision: 0,
    selected: null,
    flipped: false,
    ...overrides,
  };
}

function record(state: HandPainterState): readonly number[] {
  const canvas = new Canvas();
  new HandPainter(state).paint(canvas, SIZE);
  return canvas.finish();
}

/** 公開契約へ出た呼び出し列（「漢字が出た」のような主張を書くため）。 */
function spy(state: HandPainterState): SpyCanvas {
  const canvas = new SpyCanvas();
  new HandPainter(state).paint(canvas, SIZE);
  return canvas;
}

describe('HandPainter', () => {
  it('持ち駒があれば描き、無ければ何も描かない', () => {
    const empty = Position.newBySFEN('lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1')!;
    expect(record(stateOf()).length).toBeGreaterThan(0);
    expect(record(stateOf({ hand: empty.hand(Color.BLACK) }))).toHaveLength(0);
  });

  it('駒台にも本物の駒（五角形と漢字）が出る — 以前は文字だけだった', () => {
    const canvas = spy(stateOf());
    const held = heldTypes(WITH_HAND.hand(Color.BLACK));

    // 五角形（塗り + 縁）が駒ごとに描かれる。
    expect(canvas.calls.filter((c) => c.op === 'drawPath').length).toBeGreaterThanOrEqual(
      held.length * 2,
    );
    // 漢字も駒ごとに出る（枚数バッジは 2 枚以上のときだけ増える）。
    const glyphs = canvas.texts().map((t) => t.text);
    for (const type of held) {
      expect(glyphs, `駒台に ${String(type)} の駒字が要る`).toContain(pieceLabel(type));
    }
  });

  it('2 枚以上持っている駒だけ枚数バッジを出す', () => {
    // WITH_HAND は歩 2・銀 1。歩にだけ "2" が付く。
    const glyphs = spy(stateOf()).texts().map((t) => t.text);
    expect(glyphs).toContain('2');
    expect(glyphs).not.toContain('1');
  });

  it('選択中の駒があると記録が増える（座布団が描かれる）', () => {
    const plain = record(stateOf()).length;
    expect(record(stateOf({ selected: PieceType.PAWN })).length).toBeGreaterThan(plain);
  });

  it('相手側の駒台では駒が反転する（先手の駒台を反転盤で見たときも同じ）', () => {
    // 持ち主から見て正立、が規則。先手の駒台は flipped=false で正立、
    // flipped=true で反転する。
    const upright = record(stateOf({ flipped: false }));
    const rotated = record(stateOf({ flipped: true }));
    expect(rotated).not.toEqual(upright);
    expect(rotated.length).toBeGreaterThan(upright.length); // rotate op のぶん
  });
});

describe('HandPainter.shouldRepaint', () => {
  const base = new HandPainter(stateOf());

  it('同じ持ち駒・同じ選択なら再描画しない', () => {
    expect(new HandPainter(stateOf()).shouldRepaint(base)).toBe(false);
  });

  it('選択・反転は再描画する', () => {
    expect(new HandPainter(stateOf({ selected: PieceType.PAWN })).shouldRepaint(base)).toBe(true);
    expect(new HandPainter(stateOf({ flipped: true })).shouldRepaint(base)).toBe(true);
  });

  it('版数が進めば再描画する — 駒台は中身では判定できない', () => {
    // tsshogi の hand は局面と同じオブジェクトが**その場で**更新される。古い painter
    // が握っている参照も新しい枚数を返すので、枚数を比べる実装は「駒を取ったのに
    // 駒台が空のまま」になる。盤と同じく版数で駆動するのが正しい。
    expect(new HandPainter(stateOf({ revision: 1 })).shouldRepaint(base)).toBe(true);
  });

  it('同じ hand オブジェクトがその場更新されても版数が進めば拾う', () => {
    const live = Position.newBySFEN(
      'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1',
    )!;
    const shared = live.hand(Color.BLACK);
    const before = new HandPainter(stateOf({ hand: shared, revision: 0 }));
    // 駒を取った体で同じオブジェクトを更新する（枚数比較では検出できない状況）。
    live.hand(Color.BLACK).add(PieceType.PAWN, 1);
    const after = new HandPainter(stateOf({ hand: shared, revision: 1 }));
    expect(after.shouldRepaint(before)).toBe(true);
  });

  it('別種の painter とは常に再描画する', () => {
    expect(new HandPainter(stateOf()).shouldRepaint({ paint: () => {} })).toBe(true);
  });
});

describe('駒台の幾何 — 絵とタップ領域が同じ出所から出る', () => {
  it('枠は固定幅で等間隔に並ぶ（要素側 flex 行と同じ規則）', () => {
    const first = handSlotRect(SIZE, 0);
    const second = handSlotRect(SIZE, 1);
    expect(first.x).toBe(0);
    expect(first.width).toBe(HAND_SLOT_PX);
    // 要素側は width: HAND_SLOT_PX の子を gap: HAND_GAP_PX で並べる。
    expect(second.x - (first.x + first.width)).toBe(HAND_GAP_PX);
  });

  it('枠は縦に中央寄せされ、行より高くならない', () => {
    const tall = handSlotRect({ width: 320, height: 60 }, 0);
    expect(tall.height).toBe(HAND_SLOT_PX);
    expect(tall.y).toBeCloseTo((60 - HAND_SLOT_PX) / 2);
  });

  it('heldTypes は持っている駒種だけを安定順で返す', () => {
    const held = heldTypes(WITH_HAND.hand(Color.BLACK));
    expect(held).toContain(PieceType.PAWN);
    expect(held).not.toContain(PieceType.ROOK);
    // painter の slot 番号と要素側の子の順序が一致する根拠なので、順序も安定させる。
    expect(heldTypes(WITH_HAND.hand(Color.BLACK))).toEqual(held);
  });
});
