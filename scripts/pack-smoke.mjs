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
  tarballName,
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
const versionByName = Object.fromEntries(rows.filter((r) => r.name).map((r) => [r.name, r.version]));
const staging = mkdtempSync(join(tmpdir(), 'pack-smoke-tgz-'));
const tarballs = {};
for (const name of names) {
  run('pnpm', ['--filter', name, 'pack', '--pack-destination', staging], { cwd: repoRoot });
  const file = tarballName(name, versionByName[name]);
  if (!readdirSync(staging).includes(file)) throw new Error(`pack-smoke: expected tarball ${file} for ${name}`);
  tarballs[name] = join(staging, file);
}
log(`packed ${Object.keys(tarballs).length} tarballs`);

// 3. Install the whole closure offline in a throwaway project outside the monorepo.
const project = mkdtempSync(join(tmpdir(), 'pack-smoke-proj-'));
writeFileSync(join(project, 'package.json'), `${JSON.stringify(buildSmokeProjectManifest(tarballs), null, 2)}\n`);
// Verify each entry point's export map RESOLVES from outside the monorepo — the
// packaging guarantee (files/exports/deps are whole). We use import.meta.resolve
// rather than executing the module: @torimi/hayate-host is a bundler-consumed package
// whose transitive JSON imports need `with { type: 'json' }` under raw Node ESM,
// which is orthogonal to whether it's packaged correctly. Its real end-to-end
// execution outside the monorepo is proven by scaffold-smoke.mjs (`torimi build`).
writeFileSync(
  join(project, 'smoke.mjs'),
  `for (const name of ${JSON.stringify(SMOKE_IMPORTS)}) {\n` +
    `  const url = import.meta.resolve(name);\n` +
    `  if (!url) throw new Error('pack-smoke: could not resolve ' + name);\n` +
    `  console.log('[pack-smoke] resolved ' + name + ' -> ' + url);\n` +
    `}\n`,
);

log('installing packed closure (offline, --ignore-workspace)…');
run('pnpm', ['install', '--ignore-workspace', '--config.confirmModulesPurge=false'], { cwd: project, stdio: 'inherit' });

log('resolving smoke entry points…');
run('node', ['smoke.mjs'], { cwd: project, stdio: 'inherit' });

log('OK — public packages install and import outside the monorepo.');
