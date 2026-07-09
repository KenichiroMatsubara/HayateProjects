#!/usr/bin/env node
// check-wasm-not-stub.mjs — fail-closed guard for the release workflow (#771 / ADR-0007 §4).
// Run AFTER the real wasm build and BEFORE `pnpm -r publish`: if any public wasm
// target still carries a bootstrap stub (no real .wasm, or stub JS), exit non-zero
// so the release aborts before an un-publishable stub reaches npm.
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { loadManifest, outDirFor } from '../Hayate/scripts/wasm-manifest.mjs';

import { assessTarget, publicWasmTargets } from './check-wasm-not-stub.lib.mjs';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const hayateRoot = join(repoRoot, 'Hayate');

const manifest = loadManifest();
const verdicts = publicWasmTargets(manifest).map((target) => {
  const dir = outDirFor(target, hayateRoot);
  const wasm = join(dir, 'hayate_adapter_web_bg.wasm');
  const js = join(dir, 'hayate_adapter_web.js');
  return assessTarget({
    npmName: target.npmName,
    wasmExists: existsSync(wasm),
    jsContent: existsSync(js) ? readFileSync(js, 'utf8') : undefined,
  });
});

const stubs = verdicts.filter((v) => !v.ok);
if (stubs.length > 0) {
  console.error('check-wasm-not-stub: refusing to publish — wasm stubs detected:');
  for (const s of stubs) console.error(`  ✗ ${s.npmName}: ${s.reason}`);
  console.error('\nBuild the wasm first: pnpm --filter hayate run build:all');
  process.exit(1);
}

console.log(`check-wasm-not-stub: OK — ${verdicts.length} public wasm target(s) carry real artifacts.`);
