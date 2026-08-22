import { describe, expect, it } from 'vitest';
import type { DrawPaintPacket } from '@torimi/tsubame-renderer-protocol';
import { PieceType } from 'tsshogi';
import { paintPiece, pieceLabel } from './piece.js';
import { SpyCanvas, TRANSFORM_OPS, type DrawCall } from './test-support.js';
import type { Rect } from './board-metrics.js';

/**
 * 駒が**1 つの描画単位**であることを、painter の公開契約（`DrawCanvas`）へ
 * 出した呼び出し列だけで検証する。
 *
 * 押さえたい破れは 1 つ: 以前は五角形（painter）と漢字（要素の `text`）が別々に
 * 置かれていて、回転が図形にしか掛からなかった。**同じ変換の下で描かれる**ことを
 * 直接主張すれば、その破れは戻ってこられない。
 */

const CELL: Rect = { x: 40, y: 80, width: 40, height: 40 };

function record(type: PieceType, flipped: boolean): DrawCall[] {
  const spy = new SpyCanvas();
  paintPiece(spy, CELL, { type }, flipped);
  return spy.calls;
}

describe('paintPiece — 駒は 1 つの描画単位', () => {
  it('五角形と漢字を同じ save/restore の中に描く', () => {
    const calls = record(PieceType.PAWN, false);
    const seq = calls.map((c) => c.op);

    expect(seq.filter((o) => o === 'save')).toHaveLength(1);
    expect(seq.filter((o) => o === 'restore')).toHaveLength(1);

    const saveAt = seq.indexOf('save');
    const restoreAt = seq.indexOf('restore');
    const paths = seq.reduce<number[]>((a, o, i) => (o === 'drawPath' ? [...a, i] : a), []);
    const textAt = seq.indexOf('drawText');

    expect(paths.length).toBeGreaterThan(0);
    expect(textAt).toBeGreaterThan(saveAt);
    expect(textAt).toBeLessThan(restoreAt);
    for (const at of paths) {
      expect(at).toBeGreaterThan(saveAt);
      expect(at).toBeLessThan(restoreAt);
    }
  });

  it('図形と文字の間に変換を挟まない — 必ず同じ向きになる', () => {
    // これが以前の破れそのもの。図形と文字の間に変換が入り得るなら、
    // 「五角形だけ回って漢字は正立」が再び書けてしまう。
    for (const flipped of [false, true]) {
      const calls = record(PieceType.PAWN, flipped);
      const firstPath = calls.findIndex((c) => c.op === 'drawPath');
      const textAt = calls.findIndex((c) => c.op === 'drawText');
      const between = calls.slice(firstPath, textAt).filter((c) => TRANSFORM_OPS.has(c.op));
      expect(between, `flipped=${flipped}: no transform may sit between shape and glyph`).toEqual(
        [],
      );
    }
  });

  it('先後の違いは rotate(π) 1 つだけ', () => {
    const upright = record(PieceType.PAWN, false);
    const flipped = record(PieceType.PAWN, true);

    expect(upright.filter((c) => c.op === 'rotate')).toEqual([]);
    expect(flipped.filter((c) => c.op === 'rotate')).toEqual([
      { op: 'rotate', radians: Math.PI },
    ]);

    // 回転を除けば、出す絵は完全に同じ。**先後で地色も字も変えない** —
    // 向き以外での区別が入り込んでいたら、ここで落ちる。
    const withoutRotate = (calls: DrawCall[]): DrawCall[] => calls.filter((c) => c.op !== 'rotate');
    expect(withoutRotate(flipped)).toEqual(withoutRotate(upright));
  });

  it('駒の漢字を描く（成駒は略字）', () => {
    const text = (type: PieceType): string =>
      (record(type, false).find((c) => c.op === 'drawText') as Extract<
        DrawCall,
        { op: 'drawText' }
      >).text;

    expect(text(PieceType.PAWN)).toBe('歩');
    expect(text(PieceType.KING)).toBe('玉');
    expect(text(PieceType.PROM_PAWN)).toBe('と');
    expect(text(PieceType.PROM_KNIGHT)).toBe('圭');
    expect(text(PieceType.DRAGON)).toBe('龍');
  });

  it('成駒は字を朱で書く（向きとは独立した情報なので色で分ける）', () => {
    const inkOf = (type: PieceType): DrawPaintPacket =>
      (record(type, false).find((c) => c.op === 'drawText') as Extract<
        DrawCall,
        { op: 'drawText' }
      >).paint;

    const plain = inkOf(PieceType.SILVER).color!;
    const promoted = inkOf(PieceType.PROM_SILVER).color!;
    expect(promoted).not.toEqual(plain);
    // 朱: 赤成分が他より強い。
    expect(promoted[0]).toBeGreaterThan(promoted[1]);
    expect(promoted[0]).toBeGreaterThan(promoted[2]);
    // 生駒は無彩色に近い墨。
    expect(Math.abs(plain[0] - plain[2])).toBeLessThan(0.15);

    // 呼び出しの形（op の並び）自体は同じ — 違うのは色だけ。
    expect(record(PieceType.SILVER, false).map((c) => c.op)).toEqual(
      record(PieceType.PROM_SILVER, false).map((c) => c.op),
    );
  });

  it('字はマスの中央付近に、マスに追従する大きさで置かれる', () => {
    const text = record(PieceType.PAWN, false).find((c) => c.op === 'drawText') as Extract<
      DrawCall,
      { op: 'drawText' }
    >;
    const cx = CELL.x + CELL.width / 2;
    const cy = CELL.y + CELL.height / 2;
    // 左上原点なので、中心から字の半分ぶんだけ手前に置かれる。
    expect(text.at[0]).toBeLessThan(cx);
    expect(text.at[0]).toBeGreaterThan(CELL.x);
    expect(text.at[1]).toBeLessThan(cy);
    expect(text.at[1]).toBeGreaterThan(CELL.y);

    const big = new SpyCanvas();
    paintPiece(big, { ...CELL, width: 80, height: 80 }, { type: PieceType.PAWN }, false);
    const bigText = big.calls.find((c) => c.op === 'drawText') as Extract<
      DrawCall,
      { op: 'drawText' }
    >;
    expect(bigText.fontSize).toBeGreaterThan(text.fontSize);
  });
});

describe('pieceLabel', () => {
  it('全 14 種の駒に 1 文字の表示字を持つ', () => {
    const types = [
      PieceType.PAWN,
      PieceType.LANCE,
      PieceType.KNIGHT,
      PieceType.SILVER,
      PieceType.GOLD,
      PieceType.BISHOP,
      PieceType.ROOK,
      PieceType.KING,
      PieceType.PROM_PAWN,
      PieceType.PROM_LANCE,
      PieceType.PROM_KNIGHT,
      PieceType.PROM_SILVER,
      PieceType.HORSE,
      PieceType.DRAGON,
    ];
    for (const type of types) {
      expect([...pieceLabel(type)]).toHaveLength(1);
    }
    expect(new Set(types.map(pieceLabel)).size).toBe(types.length);
  });
});
