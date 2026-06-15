import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import type { ElementKind } from '@tsubame/renderer-protocol';
import { resolveUserSelect } from './user-select.js';

interface UserSelectParityFixture {
  name: string;
  elementKind: ElementKind;
  selectable: boolean | null;
  expected: 'text' | 'none';
}

const fixturesPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../Hayate/proto/spec/fixtures/user_select_parity.json',
);

const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8')) as UserSelectParityFixture[];

describe('user-select parity corpus (ADR-0097 / ADR-0070 single source)', () => {
  for (const fixture of fixtures) {
    it(fixture.name, () => {
      const selectable = fixture.selectable ?? undefined;
      expect(resolveUserSelect(fixture.elementKind, selectable)).toBe(fixture.expected);
    });
  }
});
