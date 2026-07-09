#!/usr/bin/env node
// pack-smoke.mjs — pack the public closure, install it in a throwaway project
// OUTSIDE the monorepo, and import the entry points an external app touches first
// (ADR-0007 acceptance: "モノレポ外で公開パッケージだけで import できる"). Run in CI
// AFTER a real wasm + JS build so the packed wasm tarballs carry real artifacts.
//
// Pure parts (the closure list, the consumer manifest shape) live in
// pack-smoke.lib.mjs and are unit-tested; this file is the process/fs orchestration.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  EXPECTED_PUBLIC_PACKAGES,
  SMOKE_IMPORTS,
  buildSmokeProjectManifest,
  publicPackages,
} from './pack-smoke.lib.mjs';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

function run(cmd, args, opts = {}) {
  return execFileSync(cmd, args, { stdio: 'pipe', encoding: 'utf8', ...opts });
}

function log(msg) {
  console.log(`[pack-smoke] ${msg}`);
}

// 1. Discover the public closure and fail fast if the workspace has drifted from
//    the intended set (a stray `private`, or a package that should stay private).
const rows = JSON.parse(run('pnpm', ['ls', '-r', '--depth', '-1', '--json'], { cwd: repoRoot }));
const names = publicPackages(rows);
const expected = [...EXPECTED_PUBLIC_PACKAGES].sort();
if (JSON.stringify(names) !== JSON.stringify(expected)) {
  const extra = names.filter((n) => !expected.includes(n));
  const missing = expected.filter((n) => !names.includes(n));
  throw new Error(
    `pack-smoke: public closure drift.\n  unexpected public: ${extra.join(', ') || '(none)'}\n  missing public:    ${missing.join(', ') || '(none)'}`,
  );
}
log(`public closure OK (${names.length} packages)`);

// 2. Pack every public package into a staging dir.
const staging = mkdtempSync(join(tmpdir(), 'pack-smoke-tgz-'));
const tarballs = {};
for (const name of names) {
  run('pnpm', ['--filter', name, 'pack', '--pack-destination', staging], { cwd: repoRoot });
  // pnpm names tarballs <sanitized-name>-<version>.tgz; match by scanning the dir.
  const file = readdirSync(staging).find((f) => f.endsWith('.tgz') && !Object.values(tarballs).some((p) => p.endsWith(f)));
  if (!file) throw new Error(`pack-smoke: could not find packed tarball for ${name}`);
  tarballs[name] = join(staging, file);
}
log(`packed ${Object.keys(tarballs).length} tarballs`);

// 3. Install the whole closure offline in a throwaway project outside the monorepo.
const project = mkdtempSync(join(tmpdir(), 'pack-smoke-proj-'));
writeFileSync(join(project, 'package.json'), `${JSON.stringify(buildSmokeProjectManifest(tarballs), null, 2)}\n`);
writeFileSync(
  join(project, 'smoke.mjs'),
  `${SMOKE_IMPORTS.map((n, i) => `import * as m${i} from ${JSON.stringify(n)};`).join('\n')}\n` +
    `for (const [name, mod] of ${JSON.stringify(SMOKE_IMPORTS)}.map((n, i) => [n, [${SMOKE_IMPORTS.map((_, i) => `m${i}`).join(', ')}][i]])) {\n` +
    `  if (!mod || typeof mod !== 'object') { throw new Error('pack-smoke: import failed for ' + name); }\n` +
    `  console.log('[pack-smoke] imported ' + name + ' (' + Object.keys(mod).length + ' exports)');\n` +
    `}\n`,
);

log('installing packed closure (offline, --ignore-workspace)…');
run('pnpm', ['install', '--ignore-workspace', '--config.confirmModulesPurge=false'], { cwd: project, stdio: 'inherit' });

log('importing smoke entry points…');
run('node', ['smoke.mjs'], { cwd: project, stdio: 'inherit' });

log('OK — public packages install and import outside the monorepo.');
