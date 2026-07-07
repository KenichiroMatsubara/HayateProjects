import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { DRAW_OP } from '@tsubame/protocol-generated/protocol';
import {
  Canvas,
  Paint,
  PaintingStyle,
  Path,
  PathFillType,
  StrokeCap,
} from '@tsubame/protocol-generated/recorder';

// #729: Flutter 流 Canvas / Path / Paint recorder。#724 の共有 roundtrip fixture の
// 期待 op 列（wire）を、記録 API で組んだ結果が再現することを検証する。
const fixturesPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../Hayate/proto/spec/fixtures/draw_encode.json',
);

interface FixtureCommand {
  readonly op: string;
  readonly [key: string]: unknown;
}
interface Fixture {
  readonly name: string;
  readonly commands: readonly FixtureCommand[];
  readonly wire: readonly number[];
}
const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8')) as Fixture[];

function fixture(name: string): Fixture {
  const f = fixtures.find((x) => x.name === name);
  if (!f) throw new Error(`missing fixture: ${name}`);
  return f;
}

const n = (v: unknown): number => v as number;

function paintFrom(p: Record<string, unknown> | undefined, style: PaintingStyle): Paint {
  const paint = new Paint();
  paint.style = style;
  if (!p) return paint;
  if (p.color !== undefined) paint.color = p.color as Paint['color'];
  if (p.fillRule !== undefined) paint.fillType = p.fillRule as PathFillType;
  if (p.strokeWidth !== undefined) paint.strokeWidth = n(p.strokeWidth);
  if (p.cap !== undefined) paint.strokeCap = p.cap as StrokeCap;
  if (p.join !== undefined) paint.strokeJoin = n(p.join);
  if (p.miterLimit !== undefined) paint.strokeMiterLimit = n(p.miterLimit);
  if (p.dash !== undefined) paint.dash = p.dash as readonly number[];
  if (p.dashOffset !== undefined) paint.dashOffset = n(p.dashOffset);
  return paint;
}

/** fixture の semantic commands を Canvas / Path / Paint で記録し、draws を返す。 */
function record(commands: readonly FixtureCommand[]): number[] {
  const canvas = new Canvas();
  let path = new Path();
  for (const c of commands) {
    switch (c.op) {
      case 'moveTo':
        path.moveTo(n(c.x), n(c.y));
        break;
      case 'lineTo':
        path.lineTo(n(c.x), n(c.y));
        break;
      case 'close':
        path.close();
        break;
      case 'quadraticTo':
        path.quadraticBezierTo(n(c.cx), n(c.cy), n(c.x), n(c.y));
        break;
      case 'cubicTo':
        path.cubicTo(n(c.c1x), n(c.c1y), n(c.c2x), n(c.c2y), n(c.x), n(c.y));
        break;
      case 'arcTo':
        path.arcTo(n(c.x1), n(c.y1), n(c.x2), n(c.y2), n(c.radius));
        break;
      case 'rect':
        path.addRect(n(c.x), n(c.y), n(c.width), n(c.height));
        break;
      case 'rrect':
        path.addRRect(n(c.x), n(c.y), n(c.width), n(c.height), n(c.rx), n(c.ry));
        break;
      case 'oval':
        path.addOval(n(c.x), n(c.y), n(c.width), n(c.height));
        break;
      case 'circle':
        path.addCircle(n(c.cx), n(c.cy), n(c.radius));
        break;
      case 'fill':
        canvas.drawPath(path, paintFrom(c.paint as Record<string, unknown>, PaintingStyle.fill));
        path = new Path();
        break;
      case 'stroke':
        canvas.drawPath(path, paintFrom(c.paint as Record<string, unknown>, PaintingStyle.stroke));
        path = new Path();
        break;
      case 'clipPath':
        canvas.clipPath(path);
        path = new Path();
        break;
      case 'save':
        canvas.save();
        break;
      case 'restore':
        canvas.restore();
        break;
      case 'translate':
        canvas.translate(n(c.dx), n(c.dy));
        break;
      case 'rotate':
        canvas.rotate(n(c.radians));
        break;
      case 'scale':
        canvas.scale(n(c.sx), n(c.sy));
        break;
      case 'transform':
        canvas.transform(n(c.a), n(c.b), n(c.c), n(c.d), n(c.e), n(c.f));
        break;
      case 'clipRect':
        canvas.clipRect(n(c.x), n(c.y), n(c.width), n(c.height));
        break;
      default:
        throw new Error(`recorder test: unknown fixture op ${c.op}`);
    }
  }
  return canvas.finish();
}

// paint が正準（fill は色のみ / evenOdd、stroke は全フィールド）な共有 fixture。
// 既定 paint（空パケット）・部分指定 stroke は Flutter Paint が常に色を持つ設計と
// ずれるため対象外（別途 default-paint の意味は decode 側が担保）。
const CANONICAL_FIXTURES = [
  'triangle-solid-fill',
  'two-fills-reset-path-between-commands',
  'quadratic-and-cubic-fill',
  'even-odd-nested-rects-leave-a-hole',
  'convenience-shapes-rrect-circle-oval',
  'arc-to-fill',
  'dashed-round-stroke-all-fields',
  'save-translate-fill-restore',
  'clip-rect-and-clip-path-then-fill',
];

describe('Canvas / Path / Paint recorder (#729)', () => {
  for (const name of CANONICAL_FIXTURES) {
    it(`records "${name}" to the shared fixture's op sequence`, () => {
      const f = fixture(name);
      expect(record(f.commands)).toEqual([...f.wire]);
    });
  }

  it('reuses a Path across multiple draws without mutating it', () => {
    const path = new Path();
    path.addRect(0, 0, 10, 10);
    const recorded = [...path.record()];

    const canvas = new Canvas();
    const paint = new Paint();
    paint.color = [1, 0, 0, 1];
    canvas.drawPath(path, paint);
    canvas.translate(20, 0);
    canvas.drawPath(path, paint); // 同じ Path を再利用

    // Path は不変: 描画しても記録済み op は変わらない。
    expect(path.record()).toEqual(recorded);
    // 同じ矩形 + fill が 2 回、間に translate が入る。
    expect(canvas.finish()).toEqual([
      DRAW_OP.RECT, 0, 0, 10, 10,
      DRAW_OP.FILL, 5, 0, 1, 0, 0, 1,
      DRAW_OP.TRANSLATE, 20, 0,
      DRAW_OP.RECT, 0, 0, 10, 10,
      DRAW_OP.FILL, 5, 0, 1, 0, 0, 1,
    ]);
  });

  it('exposes a Path method for every path verb (generated from the spec)', () => {
    const path = new Path();
    for (const m of ['moveTo', 'lineTo', 'close', 'cubicTo', 'arcTo', 'addRect', 'addCircle']) {
      expect(typeof (path as unknown as Record<string, unknown>)[m]).toBe('function');
    }
  });

  it('rejects invalid Paint values (closed vocabulary)', () => {
    const badCap = new Paint();
    badCap.style = PaintingStyle.stroke;
    badCap.strokeCap = 9 as StrokeCap;
    expect(() => badCap.toDrawPaint()).toThrow();

    const badColor = new Paint();
    badColor.color = [1, 0, 0] as unknown as Paint['color'];
    expect(() => badColor.toDrawPaint()).toThrow();

    const badWidth = new Paint();
    badWidth.style = PaintingStyle.stroke;
    badWidth.strokeWidth = -1;
    expect(() => badWidth.toDrawPaint()).toThrow();

    const outOfRange = new Paint();
    outOfRange.color = [2, 0, 0, 1];
    expect(() => outOfRange.toDrawPaint()).toThrow();
  });
});
