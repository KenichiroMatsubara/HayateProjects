import { describe, expect, it } from 'vitest';
import { Paint, PaintingStyle, Path, PathFillType, StrokeCap, StrokeJoin } from '@tsubame/protocol-generated/recorder';
import { Canvas2DReplay, type Draw2DContext } from './draw-canvas-2d.js';

// #731 / Tsubame ADR-0014: 同一 painter を CanvasRenderingContext2D へ直接 replay
// する 2D バックエンド。実ピクセルは見ず、2D コンテキストの呼び出し列を記録する
// モックで期待列との一致を検証する（意味論パリティの単位はコンテキスト呼び出し）。

type RecordedCall = [name: string, ...args: unknown[]];

function recordingCtx(): { ctx: Draw2DContext; calls: RecordedCall[] } {
  const calls: RecordedCall[] = [];
  const method =
    (name: string) =>
    (...args: unknown[]): void => {
      calls.push([name, ...args]);
    };
  const ctx = {
    save: method('save'),
    restore: method('restore'),
    translate: method('translate'),
    rotate: method('rotate'),
    scale: method('scale'),
    transform: method('transform'),
    setTransform: method('setTransform'),
    clearRect: method('clearRect'),
    beginPath: method('beginPath'),
    moveTo: method('moveTo'),
    lineTo: method('lineTo'),
    closePath: method('closePath'),
    quadraticCurveTo: method('quadraticCurveTo'),
    bezierCurveTo: method('bezierCurveTo'),
    arcTo: method('arcTo'),
    rect: method('rect'),
    roundRect: method('roundRect'),
    ellipse: method('ellipse'),
    arc: method('arc'),
    fill: method('fill'),
    stroke: method('stroke'),
    clip: method('clip'),
    setLineDash: method('setLineDash'),
    set fillStyle(v: string) {
      calls.push(['set fillStyle', v]);
    },
    set strokeStyle(v: string) {
      calls.push(['set strokeStyle', v]);
    },
    set lineWidth(v: number) {
      calls.push(['set lineWidth', v]);
    },
    set lineCap(v: string) {
      calls.push(['set lineCap', v]);
    },
    set lineJoin(v: string) {
      calls.push(['set lineJoin', v]);
    },
    set miterLimit(v: number) {
      calls.push(['set miterLimit', v]);
    },
    set lineDashOffset(v: number) {
      calls.push(['set lineDashOffset', v]);
    },
  } as unknown as Draw2DContext;
  return { ctx, calls };
}

const TWO_PI = Math.PI * 2;

describe('Canvas2DReplay: painter → 2D context call sequence (#731)', () => {
  it('replays a default-paint fill as beginPath + path verbs + fillStyle + fill(nonzero)', () => {
    const { ctx, calls } = recordingCtx();
    const canvas = new Canvas2DReplay(ctx);

    canvas.drawPath(new Path().moveTo(0, 0).lineTo(10, 0).lineTo(10, 10).close(), new Paint());

    expect(calls).toEqual([
      ['beginPath'],
      ['moveTo', 0, 0],
      ['lineTo', 10, 0],
      ['lineTo', 10, 10],
      ['closePath'],
      ['set fillStyle', 'rgba(0, 0, 0, 1)'],
      ['fill', 'nonzero'],
    ]);
  });

  it('maps the evenOdd fill rule to fill("evenodd")', () => {
    const { ctx, calls } = recordingCtx();
    const canvas = new Canvas2DReplay(ctx);
    const paint = new Paint();
    paint.color = [1, 0, 0, 0.5];
    paint.fillType = PathFillType.evenOdd;

    canvas.drawPath(new Path().addRect(0, 0, 4, 4), paint);

    expect(calls).toEqual([
      ['beginPath'],
      ['rect', 0, 0, 4, 4],
      ['set fillStyle', 'rgba(255, 0, 0, 0.5)'],
      ['fill', 'evenodd'],
    ]);
  });

  it('replays a stroke with every field mapped (width / cap / join / miterLimit / dash)', () => {
    const { ctx, calls } = recordingCtx();
    const canvas = new Canvas2DReplay(ctx);
    const paint = new Paint();
    paint.style = PaintingStyle.stroke;
    paint.color = [0, 0, 1, 1];
    paint.strokeWidth = 3;
    paint.strokeCap = StrokeCap.round;
    paint.strokeJoin = StrokeJoin.bevel;
    paint.strokeMiterLimit = 8;
    paint.dash = [4, 2];
    paint.dashOffset = 1;

    canvas.drawPath(new Path().moveTo(0, 0).lineTo(20, 0), paint);

    expect(calls).toEqual([
      ['beginPath'],
      ['moveTo', 0, 0],
      ['lineTo', 20, 0],
      ['set strokeStyle', 'rgba(0, 0, 255, 1)'],
      ['set lineWidth', 3],
      ['set lineCap', 'round'],
      ['set lineJoin', 'bevel'],
      ['set miterLimit', 8],
      ['setLineDash', [4, 2]],
      ['set lineDashOffset', 1],
      ['stroke'],
    ]);
  });

  it('resets stroke state per draw (stateless: defaults are re-applied, dash cleared)', () => {
    const { ctx, calls } = recordingCtx();
    const canvas = new Canvas2DReplay(ctx);
    const dashed = new Paint();
    dashed.style = PaintingStyle.stroke;
    dashed.dash = [4, 2];
    canvas.drawPath(new Path().moveTo(0, 0).lineTo(1, 0), dashed);
    calls.length = 0;

    const plain = new Paint();
    plain.style = PaintingStyle.stroke;
    canvas.drawPath(new Path().moveTo(0, 0).lineTo(2, 0), plain);

    expect(calls).toEqual([
      ['beginPath'],
      ['moveTo', 0, 0],
      ['lineTo', 2, 0],
      ['set strokeStyle', 'rgba(0, 0, 0, 1)'],
      ['set lineWidth', 1],
      ['set lineCap', 'butt'],
      ['set lineJoin', 'miter'],
      ['set miterLimit', 4],
      ['setLineDash', []],
      ['set lineDashOffset', 0],
      ['stroke'],
    ]);
  });

  it('maps curve verbs and convenience shapes onto their 2D equivalents', () => {
    const { ctx, calls } = recordingCtx();
    const canvas = new Canvas2DReplay(ctx);
    const path = new Path()
      .moveTo(0, 0)
      .quadraticBezierTo(1, 2, 3, 4)
      .cubicTo(1, 2, 3, 4, 5, 6)
      .arcTo(7, 8, 9, 10, 2)
      .addRRect(5, 6, 20, 10, 3, 2)
      .addOval(0, 0, 10, 6)
      .addCircle(4, 5, 2);

    canvas.drawPath(path, new Paint());

    expect(calls).toEqual([
      ['beginPath'],
      ['moveTo', 0, 0],
      ['quadraticCurveTo', 1, 2, 3, 4],
      ['bezierCurveTo', 1, 2, 3, 4, 5, 6],
      ['arcTo', 7, 8, 9, 10, 2],
      ['roundRect', 5, 6, 20, 10, [{ x: 3, y: 2 }]],
      // 楕円・円は閉じた独立 subpath として追加する（角度 0 の点へ moveTo → 全周 → close）。
      ['moveTo', 10, 3],
      ['ellipse', 5, 3, 5, 3, 0, 0, TWO_PI],
      ['closePath'],
      ['moveTo', 6, 5],
      ['arc', 4, 5, 2, 0, TWO_PI],
      ['closePath'],
      ['set fillStyle', 'rgba(0, 0, 0, 1)'],
      ['fill', 'nonzero'],
    ]);
  });

  it('maps canvas state ops: save / transforms / clipRect / clipPath / restore', () => {
    const { ctx, calls } = recordingCtx();
    const canvas = new Canvas2DReplay(ctx);

    canvas
      .save()
      .translate(10, 20)
      .rotate(Math.PI / 2)
      .scale(2, 3)
      .transform(1, 0, 0, 1, 5, 5)
      .clipRect(0, 0, 8, 8)
      .clipPath(new Path().addCircle(4, 4, 4))
      .restore();

    expect(calls).toEqual([
      ['save'],
      ['translate', 10, 20],
      ['rotate', Math.PI / 2],
      ['scale', 2, 3],
      ['transform', 1, 0, 0, 1, 5, 5],
      ['beginPath'],
      ['rect', 0, 0, 8, 8],
      ['clip'],
      ['beginPath'],
      ['moveTo', 8, 4],
      ['arc', 4, 4, 4, 0, TWO_PI],
      ['closePath'],
      ['clip'],
      ['restore'],
    ]);
  });

  it('replays the same immutable Path across multiple draws', () => {
    const { ctx, calls } = recordingCtx();
    const canvas = new Canvas2DReplay(ctx);
    const path = new Path().addRect(0, 0, 10, 10);
    const paint = new Paint();

    canvas.drawPath(path, paint);
    canvas.translate(20, 0);
    canvas.drawPath(path, paint);

    expect(calls.filter(([name]) => name === 'rect')).toEqual([
      ['rect', 0, 0, 10, 10],
      ['rect', 0, 0, 10, 10],
    ]);
  });
});
