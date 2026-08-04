import { readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';

const generatedPaths = [
  'Torimi/wire-contract/src/generated.ts',
  'Hayate/crates/platform/mobile/android/src/generated/torimi_wire.rs',
  'Hayate/crates/platform/mobile/android/cpp/generated/torimi_wire.hpp',
];

function run(command, args, cwd = process.cwd()) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

async function snapshots() {
  return Promise.all(generatedPaths.map((path) => readFile(path)));
}

const before = await snapshots();
run('node', ['Torimi/wire-contract/scripts/generate.mjs']);
const first = await snapshots();
run('node', ['Torimi/wire-contract/scripts/generate.mjs']);
const second = await snapshots();

const stale = generatedPaths.filter((_, index) => !before[index].equals(first[index]));
const nondeterministic = generatedPaths.filter((_, index) => !first[index].equals(second[index]));
if (stale.length > 0 || nondeterministic.length > 0) {
  if (stale.length > 0) console.error(`stale generated wire artifacts:\n${stale.join('\n')}`);
  if (nondeterministic.length > 0) {
    console.error(`non-deterministic generated wire artifacts:\n${nondeterministic.join('\n')}`);
  }
  process.exit(1);
}

run('pnpm', ['--filter', '@torimi/wire-contract', 'typecheck']);
run('pnpm', ['--filter', '@torimi/wire-contract', 'test']);
run('cargo', ['test', '-p', 'hayate-adapter-android', '--test', 'torimi_wire_projection'], 'Hayate');
run('c++', ['-std=c++17', '-fsyntax-only', 'Torimi/wire-contract/tests/torimi_wire_header.cpp']);
