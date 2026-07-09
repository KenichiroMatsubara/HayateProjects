// release-config.test.mjs — the changesets lockstep group must equal the public
// closure (#771 / ADR-0007 §3). If a new public package is added but not put in
// the fixed group (or vice-versa), `changeset version` would bump the train
// inconsistently — fail here instead.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { EXPECTED_PUBLIC_PACKAGES } from './pack-smoke.lib.mjs';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

test('the changesets fixed group is exactly the public closure', () => {
  const config = JSON.parse(readFileSync(join(repoRoot, '.changeset', 'config.json'), 'utf8'));
  assert.equal(config.fixed.length, 1, 'expected a single fixed lockstep group');
  assert.deepEqual([...config.fixed[0]].sort(), [...EXPECTED_PUBLIC_PACKAGES].sort());
});

test('changesets publishes scoped packages publicly', () => {
  const config = JSON.parse(readFileSync(join(repoRoot, '.changeset', 'config.json'), 'utf8'));
  assert.equal(config.access, 'public');
});
