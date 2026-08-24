import { describe, expect, it } from 'vitest';
import { Canvas } from '@torimi/tsubame-protocol-generated/recorder';
import { PieceType } from 'tsshogi';
import { CellPainter, isBlankCell, type CellPainterState } from './cell-painter.js';
import { pieceLabel } from './piece.js';
import { SpyCanvas } from './test-support.js';

const SIZE = { width: 40, height: 40 };

function stateOf(overrides: Partial<CellPainterState> = {}): CellPainterState {
  return {
    piece: null,
    flipped: false,
    mark: 'none',
    hoshi: false,
    ...overrides,
  };
}

function record(state: CellPainterState): readonly number[] {
  const canvas = new Canvas();
  new CellPainter(state).paint(canvas, SIZE);
  return canvas.finish();
}

function spy(state: CellPainterState): SpyCanvas {
  const canvas = new SpyCanvas();
  new CellPainter(state).paint(canvas, SIZE);
  return canvas;
}

describe('CellPainter', () => {
  it('駒のある枡は五角形と漢字を描く', () => {
    const canvas = spy(stateOf({ piece: { type: PieceType.ROOK } }));
    // 塗りと縁で 2 回。
    expect(canvas.calls.filter((c) => c.op === 'drawPath').length).toBeGreaterThanOrEqual(2);
    expect(canvas.texts().map((t) => t.text)).toContain(pieceLabel(PieceType.ROOK));
  });

  it('木地・罫線・ハイライトは描かない — それは要素の style が持つ', () => {
    // 何も置かれていない枡の painter は空。盤 1 枚を絵にしていた頃はここに木地と
    // 罫線が入っていた（そのぶん、どこか 1 マスが動くと全部描き直していた）。
    expect(record(stateOf())).toHaveLength(0);
    expect(isBlankCell(stateOf())).toBe(true);
  });

  it('印・星があるだけの枡でも描くものがある', () => {
    for (const state of [stateOf({ mark: 'target' }), stateOf({ hoshi: true })]) {
      expect(isBlankCell(state)).toBe(false);
      expect(record(state).length).toBeGreaterThan(0);
    }
  });

  it('印は駒の下に敷く（取れる枡の輪が駒に隠れない）', () => {
    const canvas = spy(stateOf({ piece: { type: PieceType.PAWN }, mark: 'capture' }));
    const firstPath = canvas.calls.findIndex((c) => c.op === 'drawPath');
    const firstText = canvas.calls.findIndex((c) => c.op === 'drawText');
    // 輪 → 駒（塗り・縁）→ 漢字 の順。輪が最初の drawPath。
    expect(firstPath).toBeLessThan(firstText);
    expect(canvas.calls.filter((c) => c.op === 'drawPath').length).toBeGreaterThanOrEqual(3);
  });
});

describe('CellPainter.shouldRepaint', () => {
  const base = new CellPainter(stateOf({ piece: { type: PieceType.PAWN } }));

  it('同じ枡の中身なら再描画しない — 隣の枡が動いても自分は無関係', () => {
    expect(new CellPainter(stateOf({ piece: { type: PieceType.PAWN } })).shouldRepaint(base)).toBe(
      false,
    );
  });

  it('駒が変わる・消える・成る・向きが変わるのはすべて再描画', () => {
    const changed: CellPainterState[] = [
      stateOf({ piece: null }),
      stateOf({ piece: { type: PieceType.PROM_PAWN } }),
      stateOf({ piece: { type: PieceType.PAWN }, flipped: true }),
      stateOf({ piece: { type: PieceType.PAWN }, mark: 'capture' }),
      stateOf({ piece: { type: PieceType.PAWN }, hoshi: true }),
    ];
    for (const state of changed) {
      expect(new CellPainter(state).shouldRepaint(base)).toBe(true);
    }
  });

  it('駒を取っても向きが変わるので拾える（同じ駒種でも見分けがつく）', () => {
    // 歩を歩で取ると駒種は同じまま。先後が入れ替わる（＝向きが変わる）ので、
    // 駒種だけでなく向きも再描画キーに入れておく必要がある。
    const before = new CellPainter(stateOf({ piece: { type: PieceType.PAWN }, flipped: true }));
    const after = new CellPainter(stateOf({ piece: { type: PieceType.PAWN }, flipped: false }));
    expect(after.shouldRepaint(before)).toBe(true);
  });

  it('別種の painter とは常に再描画する', () => {
    expect(new CellPainter(stateOf()).shouldRepaint({ paint: () => {} })).toBe(true);
  });
});
