import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  appendDrawArcTo,
  appendDrawCircle,
  appendDrawClipPath,
  appendDrawClipRect,
  appendDrawClose,
  appendDrawCubicTo,
  appendDrawFill,
  appendDrawLineTo,
  appendDrawMoveTo,
  appendDrawOval,
  appendDrawQuadraticTo,
  appendDrawRect,
  appendDrawRestore,
  appendDrawRotate,
  appendDrawRrect,
  appendDrawSave,
  appendDrawScale,
  appendDrawStroke,
  appendDrawTransform,
  appendDrawTranslate,
  type DrawPaint,
} from '@torimi/tsubame-protocol-generated/codec';

// draw display list の codec fixture（#724 / ADR-0142）。TS encode（本テスト）と
// Rust decode（Hayate/crates/core/tests/draw_codec_fixtures.rs）が同じ fixture を
// 共有し、encode ↔ decode の drift を機械検出する。
const fixturesPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../Hayate/proto/spec/fixtures/draw_encode.json',
);

interface DrawFixtureCommand {
  readonly op:
    | 'moveTo'
    | 'lineTo'
    | 'close'
    | 'fill'
    | 'stroke'
    | 'quadraticTo'
    | 'cubicTo'
    | 'arcTo'
    | 'rect'
    | 'rrect'
    | 'oval'
    | 'circle'
    | 'save'
    | 'restore'
    | 'translate'
    | 'rotate'
    | 'scale'
    | 'transform'
    | 'clipRect'
    | 'clipPath';
  readonly x?: number;
  readonly y?: number;
  readonly cx?: number;
  readonly cy?: number;
  readonly dx?: number;
  readonly dy?: number;
  readonly sx?: number;
  readonly sy?: number;
  readonly radians?: number;
  readonly a?: number;
  readonly b?: number;
  readonly c?: number;
  readonly d?: number;
  readonly e?: number;
  readonly f?: number;
  readonly c1x?: number;
  readonly c1y?: number;
  readonly c2x?: number;
  readonly c2y?: number;
  readonly x1?: number;
  readonly y1?: number;
  readonly x2?: number;
  readonly y2?: number;
  readonly radius?: number;
  readonly width?: number;
  readonly height?: number;
  readonly rx?: number;
  readonly ry?: number;
  readonly paint?: DrawPaint;
}

interface DrawFixture {
  readonly name: string;
  readonly commands: readonly DrawFixtureCommand[];
  readonly wire: readonly number[];
}

const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8')) as DrawFixture[];

function encodeCommands(commands: readonly DrawFixtureCommand[]): number[] {
  const draws: number[] = [];
  for (const command of commands) {
    switch (command.op) {
      case 'moveTo':
        appendDrawMoveTo(draws, command.x!, command.y!);
        break;
      case 'lineTo':
        appendDrawLineTo(draws, command.x!, command.y!);
        break;
      case 'close':
        appendDrawClose(draws);
        break;
      case 'quadraticTo':
        appendDrawQuadraticTo(draws, command.cx!, command.cy!, command.x!, command.y!);
        break;
      case 'cubicTo':
        appendDrawCubicTo(
          draws,
          command.c1x!,
          command.c1y!,
          command.c2x!,
          command.c2y!,
          command.x!,
          command.y!,
        );
        break;
      case 'arcTo':
        appendDrawArcTo(draws, command.x1!, command.y1!, command.x2!, command.y2!, command.radius!);
        break;
      case 'rect':
        appendDrawRect(draws, command.x!, command.y!, command.width!, command.height!);
        break;
      case 'rrect':
        appendDrawRrect(
          draws,
          command.x!,
          command.y!,
          command.width!,
          command.height!,
          command.rx!,
          command.ry!,
        );
        break;
      case 'oval':
        appendDrawOval(draws, command.x!, command.y!, command.width!, command.height!);
        break;
      case 'circle':
        appendDrawCircle(draws, command.cx!, command.cy!, command.radius!);
        break;
      case 'fill':
        appendDrawFill(draws, command.paint ?? {});
        break;
      case 'stroke':
        appendDrawStroke(draws, command.paint ?? {});
        break;
      case 'save':
        appendDrawSave(draws);
        break;
      case 'restore':
        appendDrawRestore(draws);
        break;
      case 'translate':
        appendDrawTranslate(draws, command.dx!, command.dy!);
        break;
      case 'rotate':
        appendDrawRotate(draws, command.radians!);
        break;
      case 'scale':
        appendDrawScale(draws, command.sx!, command.sy!);
        break;
      case 'transform':
        appendDrawTransform(
          draws,
          command.a!,
          command.b!,
          command.c!,
          command.d!,
          command.e!,
          command.f!,
        );
        break;
      case 'clipRect':
        appendDrawClipRect(draws, command.x!, command.y!, command.width!, command.height!);
        break;
      case 'clipPath':
        appendDrawClipPath(draws);
        break;
    }
  }
  return draws;
}

describe('draw codec fixtures (TS encode ↔ Rust decode)', () => {
  it('has at least one fixture', () => {
    expect(fixtures.length).toBeGreaterThan(0);
  });

  for (const fixture of fixtures) {
    it(fixture.name, () => {
      expect(encodeCommands(fixture.commands)).toEqual(fixture.wire);
    });
  }
});
