#!/usr/bin/env node
// scaffold-smoke.mjs — the ADR-0008 §6 acceptance made executable: using ONLY the
// packed public tarballs (no workspace), run `create-torimi` OUTSIDE the monorepo,
// then `torimi build` for both targets. Proves an external developer can go from
// `npm create torimi` to a built App Bundle with published packages alone.
//
// Run in CI AFTER a real wasm + JS build (the packed wasm tarballs must carry real
// artifacts). Reuses the pack/closure logic from pack-smoke.lib.mjs.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, writeFileSync, mkdirSync, cpSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  EXPECTED_PUBLIC_PACKAGES,
  buildSmokeWorkspaceConfig,
  publicPackages,
  tarballName,
} from './pack-smoke.lib.mjs';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

function run(cmd, args, opts = {}) {
  return execFileSync(cmd, args, { stdio: 'pipe', encoding: 'utf8', ...opts });
}
function log(msg) {
  console.log(`[scaffold-smoke] ${msg}`);
}

// 1. Pack the whole public closure (create-torimi included).
const rows = JSON.parse(run('pnpm', ['ls', '-r', '--depth', '-1', '--json'], { cwd: repoRoot }));
const names = publicPackages(rows);
if (JSON.stringify(names) !== JSON.stringify([...EXPECTED_PUBLIC_PACKAGES].sort())) {
  throw new Error('scaffold-smoke: public closure drift — run pack-smoke for details');
}
const versionByName = Object.fromEntries(rows.filter((r) => r.name).map((r) => [r.name, r.version]));
const staging = mkdtempSync(join(tmpdir(), 'scaffold-smoke-tgz-'));
const tarballs = {};
for (const name of names) {
  run('pnpm', ['--filter', name, 'pack', '--pack-destination', staging], { cwd: repoRoot });
  const file = tarballName(name, versionByName[name]);
  if (!readdirSync(staging).includes(file)) throw new Error(`scaffold-smoke: expected tarball ${file} for ${name}`);
  tarballs[name] = join(staging, file);
}
log(`packed ${Object.keys(tarballs).length} tarballs`);

// 2. Scaffold OUTSIDE the monorepo using the packed create-torimi (extract its
//    tarball so we run exactly what npm would install — bundled template included).
const work = mkdtempSync(join(tmpdir(), 'scaffold-smoke-'));
const createDir = join(work, 'create-torimi');
mkdirSync(createDir, { recursive: true });
run('tar', ['-xzf', tarballs['create-torimi'], '-C', createDir, '--strip-components=1']);
const projectName = 'smoke-app';
run('node', [join(createDir, 'dist', 'cli.js'), projectName], { cwd: work, stdio: 'inherit' });
const projectDir = join(work, projectName);
log(`scaffolded ${projectName}`);

// 3. Pin every public package to its local tarball via workspace-root pnpm
//    overrides so the whole closure installs offline — exactly what a registry
//    install would resolve. pnpm 11 no longer reads `pnpm.overrides` from package.json.
const pkg = JSON.parse(readFileSync(join(projectDir, 'package.json'), 'utf8'));
writeFileSync(join(projectDir, 'package.json'), `${JSON.stringify(pkg, null, 2)}\n`);
writeFileSync(join(projectDir, 'pnpm-workspace.yaml'), buildSmokeWorkspaceConfig(tarballs));

log('installing scaffolded project (offline, packed tarballs only)…');
run('pnpm', ['install', '--config.confirmModulesPurge=false'], { cwd: projectDir, stdio: 'inherit' });

for (const target of ['web', 'native']) {
  log(`torimi build ${target}…`);
  run('pnpm', ['exec', 'torimi', 'build', target], { cwd: projectDir, stdio: 'inherit' });
}

log('OK — create-torimi scaffold builds both targets outside the monorepo.');
