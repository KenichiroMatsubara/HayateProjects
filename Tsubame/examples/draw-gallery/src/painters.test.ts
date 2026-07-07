import { describe, it, expect } from 'vitest';
import { DRAW_OP } from '@tsubame/protocol-generated/protocol';
import {
  PaintingStyle,
  PathFillType,
  StrokeCap,
  StrokeJoin,
} from '@tsubame/protocol-generated/recorder';
import type {
  DrawCanvas,
  DrawRecordedPath,
  DrawPaintSource,
  DrawPaintPacket,
  DrawSize,
} from '@tsubame/renderer-protocol';
import {
  curveChart,
  evenOddDonut,
  dashStrokeSampler,
  rotatedClip,
  responsiveGrid,
  GALLERY_PAINTERS,
} from './painters.js';

/**
 * painter を「観測可能な振る舞い」で検証するための最小 DrawCanvas スパイ。
 * painter が公開契約（DrawCanvas）に対して発行した呼び出し列だけを記録する
 * ので、どちらのレンダラーの内部実装にも依存しない（recorder / canvas 2D の
 * どちらでもない）。tests.md の「公開インターフェース越しに振る舞いを見る」流儀。
 */
type DrawCall =
  | { op: 'drawPath'; ops: readonly number[]; style: number; paint: DrawPaintPacket }
  | { op: 'clipPath'; ops: readonly number[] }
  | { op: 'clipRect'; rect: readonly [number, number, number, number] }
  | { op: 'save' }
  | { op: 'restore' }
  | { op: 'translate'; args: readonly [number, number] }
  | { op: 'rotate'; radians: number }
  | { op: 'scale'; args: readonly [number, number] }
  | { op: 'transform'; args: readonly [number, number, number, number, number, number] };

class SpyCanvas implements DrawCanvas {
  readonly calls: DrawCall[] = [];

  drawPath(path: DrawRecordedPath, paint: DrawPaintSource): this {
    this.calls.push({
      op: 'drawPath',
      ops: [...path.record()],
      style: paint.style,
      paint: paint.toDrawPaint(),
    });
    return this;
  }
  clipPath(path: DrawRecordedPath): this {
    this.calls.push({ op: 'clipPath', ops: [...path.record()] });
    return this;
  }
  clipRect(x: number, y: number, width: number, height: number): this {
    this.calls.push({ op: 'clipRect', rect: [x, y, width, height] });
    return this;
  }
  save(): this {
    this.calls.push({ op: 'save' });
    return this;
  }
  restore(): this {
    this.calls.push({ op: 'restore' });
    return this;
  }
  translate(dx: number, dy: number): this {
    this.calls.push({ op: 'translate', args: [dx, dy] });
    return this;
  }
  rotate(radians: number): this {
    this.calls.push({ op: 'rotate', radians });
    return this;
  }
  scale(sx: number, sy: number): this {
    this.calls.push({ op: 'scale', args: [sx, sy] });
    return this;
  }
  transform(a: number, b: number, c: number, d: number, e: number, f: number): this {
    this.calls.push({ op: 'transform', args: [a, b, c, d, e, f] });
    return this;
  }
}

/** display list の op 列に指定 op が現れるか（引数はスキップして op コードだけ走査）。 */
function countPathOp(ops: readonly number[], target: number): number {
  const slots: Record<number, number> = {
    [DRAW_OP.MOVE_TO]: 2,
    [DRAW_OP.LINE_TO]: 2,
    [DRAW_OP.CLOSE]: 0,
    [DRAW_OP.QUADRATIC_TO]: 4,
    [DRAW_OP.CUBIC_TO]: 6,
    [DRAW_OP.ARC_TO]: 5,
    [DRAW_OP.RECT]: 4,
    [DRAW_OP.RRECT]: 6,
    [DRAW_OP.OVAL]: 4,
    [DRAW_OP.CIRCLE]: 3,
  };
  let count = 0;
  for (let i = 0; i < ops.length; ) {
    const op = ops[i]!;
    if (op === target) count++;
    const slot = slots[op];
    if (slot === undefined) throw new Error(`unknown path op ${op}`);
    i += 1 + slot;
  }
  return count;
}

function pathHasOp(ops: readonly number[], target: number): boolean {
  const slots: Record<number, number> = {
    [DRAW_OP.MOVE_TO]: 2,
    [DRAW_OP.LINE_TO]: 2,
    [DRAW_OP.CLOSE]: 0,
    [DRAW_OP.QUADRATIC_TO]: 4,
    [DRAW_OP.CUBIC_TO]: 6,
    [DRAW_OP.ARC_TO]: 5,
    [DRAW_OP.RECT]: 4,
    [DRAW_OP.RRECT]: 6,
    [DRAW_OP.OVAL]: 4,
    [DRAW_OP.CIRCLE]: 3,
  };
  for (let i = 0; i < ops.length; ) {
    const op = ops[i]!;
    if (op === target) return true;
    const slot = slots[op];
    if (slot === undefined) throw new Error(`unknown path op ${op}`);
    i += 1 + slot;
  }
  return false;
}

const SIZE: DrawSize = { width: 200, height: 120 };

describe('curveChart painter (cubic bezier)', () => {
  it('strokes a single smooth cubic-bezier path across the box', () => {
    const spy = new SpyCanvas();
    curveChart(spy, SIZE);

    const draws = spy.calls.filter((c) => c.op === 'drawPath');
    expect(draws).toHaveLength(1);
    const curve = draws[0] as Extract<DrawCall, { op: 'drawPath' }>;
    expect(curve.style).toBe(PaintingStyle.stroke);
    expect(pathHasOp(curve.ops, DRAW_OP.CUBIC_TO)).toBe(true);
  });

  it('scales the curve to the box: the path reaches the right edge', () => {
    const wide = new SpyCanvas();
    curveChart(wide, { width: 400, height: 100 });
    const narrow = new SpyCanvas();
    curveChart(narrow, { width: 100, height: 100 });

    const maxX = (spy: SpyCanvas): number => {
      const ops = (spy.calls.find((c) => c.op === 'drawPath') as
        | Extract<DrawCall, { op: 'drawPath' }>
        | undefined)!.ops;
      // x 座標は各 vertex の第1引数。緩く「最大値がおよそ幅」を確認する。
      let max = 0;
      for (let i = 0; i < ops.length; i++) if (ops[i]! > max) max = ops[i]!;
      return max;
    };
    expect(maxX(wide)).toBeGreaterThan(maxX(narrow));
    expect(maxX(wide)).toBeGreaterThanOrEqual(360);
  });
});

describe('evenOddDonut painter (evenOdd hole)', () => {
  it('fills two nested rings with the evenOdd rule so the center is punched out', () => {
    const spy = new SpyCanvas();
    evenOddDonut(spy, SIZE);

    const draws = spy.calls.filter((c) => c.op === 'drawPath') as Extract<
      DrawCall,
      { op: 'drawPath' }
    >[];
    expect(draws).toHaveLength(1);
    const ring = draws[0] as Extract<DrawCall, { op: 'drawPath' }>;
    expect(ring.style).toBe(PaintingStyle.fill);
    // 穴あきは evenOdd に依存する（nonZero だと同巻きで塗り潰されて穴が出ない）。
    expect(ring.paint.fillRule).toBe(PathFillType.evenOdd);
    // 外周と内周の 2 つの閉じたサブパス（円）を 1 パスに含む。
    expect(countPathOp(ring.ops, DRAW_OP.CIRCLE)).toBe(2);
  });
});

describe('dashStrokeSampler painter (dash + cap/join)', () => {
  it('strokes several sample lines and exercises dash, distinct caps, and distinct joins', () => {
    const spy = new SpyCanvas();
    dashStrokeSampler(spy, SIZE);

    const draws = spy.calls.filter((c) => c.op === 'drawPath') as Extract<
      DrawCall,
      { op: 'drawPath' }
    >[];
    expect(draws.length).toBeGreaterThanOrEqual(3);
    expect(draws.every((d) => d.style === PaintingStyle.stroke)).toBe(true);

    // 少なくとも 1 本は破線（dash 配列が非空）。
    expect(draws.some((d) => (d.paint.dash?.length ?? 0) > 0)).toBe(true);

    // cap / join の見本: round と square の両方、非 miter の join を含む。
    const caps = new Set(draws.map((d) => d.paint.cap));
    expect(caps.has(StrokeCap.round)).toBe(true);
    expect(caps.has(StrokeCap.square)).toBe(true);
    const joins = new Set(draws.map((d) => d.paint.join));
    expect(joins.has(StrokeJoin.round) || joins.has(StrokeJoin.bevel)).toBe(true);
  });
});

describe('rotatedClip painter (rotate + clip)', () => {
  it('clips to a rotated region: save → rotate → clip → draw → restore, balanced', () => {
    const spy = new SpyCanvas();
    rotatedClip(spy, SIZE);

    const seq = spy.calls.map((c) => c.op);
    // save/restore が釣り合う（変換・クリップが漏れない）。
    expect(seq.filter((o) => o === 'save').length).toBe(
      seq.filter((o) => o === 'restore').length,
    );

    const saveAt = seq.indexOf('save');
    const rotateAt = seq.indexOf('rotate');
    const clipAt = seq.findIndex((o) => o === 'clipRect' || o === 'clipPath');
    const drawAt = seq.indexOf('drawPath');
    const restoreAt = seq.lastIndexOf('restore');

    expect(saveAt).toBeGreaterThanOrEqual(0);
    expect(rotateAt).toBeGreaterThan(saveAt);
    expect(clipAt).toBeGreaterThan(rotateAt);
    expect(drawAt).toBeGreaterThan(clipAt);
    expect(restoreAt).toBeGreaterThan(drawAt);

    const rotate = spy.calls[rotateAt] as Extract<DrawCall, { op: 'rotate' }>;
    expect(rotate.radians).not.toBe(0);
  });
});

describe('responsiveGrid painter (size-following)', () => {
  it('draws more cells as the box grows — the picture changes with size, not just scales', () => {
    const small = new SpyCanvas();
    responsiveGrid(small, { width: 120, height: 120 });
    const large = new SpyCanvas();
    responsiveGrid(large, { width: 480, height: 480 });

    const cells = (spy: SpyCanvas): number =>
      spy.calls.filter((c) => c.op === 'drawPath').length;

    expect(cells(small)).toBeGreaterThan(0);
    expect(cells(large)).toBeGreaterThan(cells(small));
  });

  it('draws nothing for a degenerate zero-size box (no paint before first layout)', () => {
    const spy = new SpyCanvas();
    responsiveGrid(spy, { width: 0, height: 0 });
    expect(spy.calls).toHaveLength(0);
  });
});

describe('GALLERY_PAINTERS registry', () => {
  it('exposes every sample painter with a unique id and each paints something at a normal size', () => {
    expect(GALLERY_PAINTERS.length).toBeGreaterThanOrEqual(5);
    const ids = GALLERY_PAINTERS.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    // 受け入れ基準の 5 種を id で担保する。
    expect(ids).toEqual(
      expect.arrayContaining(['curve-chart', 'even-odd-donut', 'dash-sampler', 'rotated-clip', 'responsive-grid']),
    );
    for (const entry of GALLERY_PAINTERS) {
      const spy = new SpyCanvas();
      entry.paint(spy, SIZE);
      expect(spy.calls.length, `${entry.id} should paint at ${SIZE.width}x${SIZE.height}`).toBeGreaterThan(0);
    }
  });
});
