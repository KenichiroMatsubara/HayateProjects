import { describe, expect, it } from 'vitest';
import { Canvas } from '@torimi/tsubame-protocol-generated/recorder';
import { Position, Square } from 'tsshogi';
import { BoardPainter, type BoardPainterState } from './board-painter.js';
import { boardMetrics, indexAtScreenOrder, squareRect } from './board-metrics.js';

const SIZE = { width: 360, height: 360 };

function stateOf(overrides: Partial<BoardPainterState> = {}): BoardPainterState {
  return {
    position: Position.newBySFEN(
      'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1',
    )!,
    revision: 0,
    selected: -1,
    targets: [],
    lastFrom: -1,
    lastTo: -1,
    checkedKing: -1,
    flipped: false,
    ...overrides,
  };
}

function record(state: BoardPainterState): readonly number[] {
  const canvas = new Canvas();
  new BoardPainter(state).paint(canvas, SIZE);
  return canvas.finish();
}

describe('BoardPainter', () => {
  it('初期局面で空でない display list を記録する', () => {
    expect(record(stateOf()).length).toBeGreaterThan(0);
  });

  it('ハイライトを足すと記録が増える（描かれている証拠）', () => {
    const plain = record(stateOf()).length;
    const highlighted = record(
      stateOf({ selected: 60, targets: [51], lastFrom: 62, lastTo: 53, checkedKing: 76 }),
    ).length;
    expect(highlighted).toBeGreaterThan(plain);
  });

  it('駒の無い盤は駒のある盤より記録が短い', () => {
    const empty = record(stateOf({ position: Position.newBySFEN('9/9/9/9/9/9/9/9/9 b - 1')! }));
    expect(empty.length).toBeLessThan(record(stateOf()).length);
  });
});

describe('BoardPainter.shouldRepaint', () => {
  const base = new BoardPainter(stateOf());

  it('同じキーなら再描画しない', () => {
    expect(new BoardPainter(stateOf()).shouldRepaint(base)).toBe(false);
  });

  it('局面が進めば再描画する', () => {
    expect(new BoardPainter(stateOf({ revision: 1 })).shouldRepaint(base)).toBe(true);
  });

  it('選択・直前手・反転はいずれも再描画する', () => {
    expect(new BoardPainter(stateOf({ selected: 60 })).shouldRepaint(base)).toBe(true);
    expect(new BoardPainter(stateOf({ lastTo: 53 })).shouldRepaint(base)).toBe(true);
    expect(new BoardPainter(stateOf({ flipped: true })).shouldRepaint(base)).toBe(true);
  });

  it('候補マスは revision と selected から導出されるので単独ではキーに入らない', () => {
    expect(new BoardPainter(stateOf({ targets: [1, 2, 3] })).shouldRepaint(base)).toBe(false);
  });

  it('別種の painter とは常に再描画する', () => {
    const foreign = { paint: () => {} };
    expect(new BoardPainter(stateOf()).shouldRepaint(foreign)).toBe(true);
  });
});

describe('board-metrics', () => {
  it('枡目は箱を 9 等分する — 要素側の 9×9 grid とちょうど重なる', () => {
    // 要素側は `board` の箱をそのまま 9 等分した grid でタップ領域を敷く。
    // 枡目を外枠のぶん内側へ寄せると、絵と当たり判定がその分ずれる（駒の絵は
    // painter 側、タップは要素側なので、ずれは目に見えて操作に出る）。
    const metrics = boardMetrics(SIZE);
    expect(metrics.grid).toEqual(metrics.board);
    expect(metrics.cell).toBeCloseTo(SIZE.width / 9);
    for (const order of [0, 1, 40, 79, 80]) {
      const rect = squareRect(metrics, order, false);
      expect(rect.x).toBeCloseTo((order % 9) * (SIZE.width / 9));
      expect(rect.y).toBeCloseTo(Math.floor(order / 9) * (SIZE.height / 9));
    }
  });

  it('81マスが盤の内側をちょうど敷き詰める', () => {
    const metrics = boardMetrics(SIZE);
    const first = squareRect(metrics, 0, false);
    const last = squareRect(metrics, 80, false);
    expect(first.x).toBeCloseTo(metrics.grid.x);
    expect(first.y).toBeCloseTo(metrics.grid.y);
    expect(last.x + last.width).toBeCloseTo(metrics.grid.x + metrics.grid.width);
    expect(last.y + last.height).toBeCloseTo(metrics.grid.y + metrics.grid.height);
    expect(metrics.cell * 9).toBeCloseTo(metrics.grid.width);
  });

  it('反転は対合（二度掛けると元に戻る）', () => {
    for (const order of [0, 17, 40, 63, 80]) {
      expect(indexAtScreenOrder(indexAtScreenOrder(order, true), true)).toBe(order);
      expect(indexAtScreenOrder(order, false)).toBe(order);
    }
  });

  it('反転すると 9一 と 1九 が入れ替わる', () => {
    const metrics = boardMetrics(SIZE);
    const nine1 = new Square(9, 1).index;
    expect(squareRect(metrics, nine1, false)).toEqual(squareRect(metrics, 80 - nine1, true));
  });

  it('非正方形の box でも短辺に合わせて中央寄せする', () => {
    const metrics = boardMetrics({ width: 400, height: 300 });
    expect(metrics.board.width).toBe(300);
    expect(metrics.board.x).toBeCloseTo(50);
    expect(metrics.board.y).toBeCloseTo(0);
  });
});
