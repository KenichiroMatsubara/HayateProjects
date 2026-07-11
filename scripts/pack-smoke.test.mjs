// pack-smoke.test.mjs — unit tests for the pack smoke closure logic (#768).
// Run with: node --test scripts/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  EXPECTED_PUBLIC_PACKAGES,
  SMOKE_IMPORTS,
  buildSmokeProjectManifest,
  publicPackages,
  tarballName,
} from './pack-smoke.lib.mjs';

test('tarballName matches npm scheme and disambiguates torimi from create-torimi', () => {
  assert.equal(tarballName('@torimi/hayate-host', '0.1.0'), 'hayate-host-0.1.0.tgz');
  assert.equal(tarballName('torimi', '0.1.0'), 'torimi-0.1.0.tgz');
  assert.equal(tarballName('create-torimi', '0.1.0'), 'create-torimi-0.1.0.tgz');
  // the two must be distinct filenames (the endsWith bug they replace)
  assert.notEqual(tarballName('torimi', '0.1.0'), tarballName('create-torimi', '0.1.0'));
});

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

test('publicPackages keeps non-private named rows and drops the rest', () => {
  const rows = [
    { name: '@torimi/hayate-host', private: false },
    { name: 'hayate-adapter-web-null', private: true },
    { name: 'hayate-projects', private: true },
    { private: false }, // unnamed root-ish row — skipped
    { name: '@torimi/bundle', private: false },
  ];
  assert.deepEqual(publicPackages(rows), ['@torimi/hayate-host', '@torimi/bundle']);
});

test('the SMOKE_IMPORTS are all inside the public closure', () => {
  for (const name of SMOKE_IMPORTS) {
    assert.ok(EXPECTED_PUBLIC_PACKAGES.includes(name), `${name} must be public`);
  }
});

test('buildSmokeProjectManifest pins every tarball via overrides and depends on the smoke imports', () => {
  const tarballs = {
    '@torimi/tsubame-solid': '/tmp/a.tgz',
    '@torimi/hayate-host': '/tmp/b.tgz',
    '@torimi/dev-server': '/tmp/c.tgz',
    '@torimi/hayate-adapter-web': '/tmp/d.tgz',
  };
  const manifest = buildSmokeProjectManifest(tarballs);

  assert.equal(manifest.private, true);
  // Every tarball is pinned as an override so inter-package deps resolve to the
  // local tarballs, never the network.
  assert.deepEqual(manifest.pnpm.overrides, {
    '@torimi/tsubame-solid': 'file:/tmp/a.tgz',
    '@torimi/hayate-host': 'file:/tmp/b.tgz',
    '@torimi/dev-server': 'file:/tmp/c.tgz',
    '@torimi/hayate-adapter-web': 'file:/tmp/d.tgz',
  });
  // The three smoke imports are direct file: deps.
  assert.deepEqual(manifest.dependencies, {
    '@torimi/tsubame-solid': 'file:/tmp/a.tgz',
    '@torimi/hayate-host': 'file:/tmp/b.tgz',
    '@torimi/dev-server': 'file:/tmp/c.tgz',
  });
});

test('buildSmokeProjectManifest throws if a smoke import was not packed', () => {
  assert.throws(() => buildSmokeProjectManifest({ '@torimi/tsubame-solid': '/tmp/a.tgz' }), /no packed tarball/);
});

// The guardrail that makes #768 stick: the workspace's actual public/private
// split must equal the intended closure. A package that forgets to drop
// `private` (or keeps it wrongly) fails here, in a test that runs without a wasm
// build — no need to reach the CI pack job to catch the mistake.
test('the workspace public closure matches EXPECTED_PUBLIC_PACKAGES', () => {
  const rows = JSON.parse(execFileSync('pnpm', ['ls', '-r', '--depth', '-1', '--json'], { cwd: repoRoot, encoding: 'utf8' }));
  assert.deepEqual(publicPackages(rows), [...EXPECTED_PUBLIC_PACKAGES].sort());
});
