import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  appendDrawClose,
  appendDrawFill,
  appendDrawLineTo,
  appendDrawMoveTo,
  type DrawPaint,
} from '@tsubame/protocol-generated/codec';

// draw display list の codec fixture（#724 / ADR-0142）。TS encode（本テスト）と
// Rust decode（Hayate/crates/core/tests/draw_codec_fixtures.rs）が同じ fixture を
// 共有し、encode ↔ decode の drift を機械検出する。
const fixturesPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../Hayate/proto/spec/fixtures/draw_encode.json',
);

interface DrawFixtureCommand {
  readonly op: 'moveTo' | 'lineTo' | 'close' | 'fill';
  readonly x?: number;
  readonly y?: number;
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
      case 'fill':
        appendDrawFill(draws, command.paint ?? {});
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
