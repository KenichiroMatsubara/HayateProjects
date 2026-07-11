import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import type { StylePatch } from '@torimi/tsubame-renderer-protocol';
import { encodeStylePatch } from '@torimi/tsubame-protocol-generated/codec';

const fixturesPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../Hayate/proto/spec/fixtures/style_encode.json',
);

interface StyleFixture {
  readonly name: string;
  readonly patch: StylePatch;
  readonly wire: readonly number[];
}

const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8')) as StyleFixture[];

describe('codec fixtures (C2)', () => {
  for (const fixture of fixtures) {
    it(fixture.name, () => {
      const out: number[] = [];
      encodeStylePatch(fixture.patch, out);
      expect(out).toEqual(fixture.wire);
    });
  }
});
