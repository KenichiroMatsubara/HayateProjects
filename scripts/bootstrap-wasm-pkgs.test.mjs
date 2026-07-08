// bootstrap-wasm-pkgs.test.mjs — unit tests for the manifest-driven stub generator.
// Run with: node --test scripts/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { bootstrapWasmPkgs } from './bootstrap-wasm-pkgs.lib.mjs';

async function withTempHayateRoot(fn) {
  const hayateRoot = await mkdtemp(join(tmpdir(), 'bootstrap-wasm-pkgs-test-'));
  try {
    await fn(hayateRoot);
  } finally {
    await rm(hayateRoot, { recursive: true, force: true });
  }
}

function fakeTarget(name, npmName = `hayate-adapter-web-${name}`) {
  return {
    name,
    npmName,
    outDir: `wasm-pkgs/${name}`,
    targetDir: `target/wasm-${name}`,
    cargoFeatures: { mode: 'inherit', names: [] },
    description: 'x',
    includeInDefaultBuild: true,
  };
}

function fakeManifest(targets) {
  return { crateDir: 'crates/platform/web', npmPackageName: 'hayate-adapter-web', targets };
}

test('creates a stub package for every manifest target', () =>
  withTempHayateRoot(async (hayateRoot) => {
    const manifest = fakeManifest([fakeTarget('pkg'), fakeTarget('pkg-tiny-skia')]);

    await bootstrapWasmPkgs({ hayateRoot, manifest });

    for (const name of ['pkg', 'pkg-tiny-skia']) {
      const dir = join(hayateRoot, 'wasm-pkgs', name);
      const pkgJson = JSON.parse(await readFile(join(dir, 'package.json'), 'utf8'));
      // The stub carries the target's own npmName, so pnpm links each alias to
      // its source dir instead of colliding on a shared name (#765).
      assert.equal(pkgJson.name, `hayate-adapter-web-${name}`);
      const js = await readFile(join(dir, 'hayate_adapter_web.js'), 'utf8');
      assert.match(js, /WASM not built/);
      await readFile(join(dir, 'hayate_adapter_web.d.ts'), 'utf8'); // does not throw
      await readFile(join(dir, '.gitignore'), 'utf8'); // does not throw
    }
  }));

test('leaves an already-built target (real .wasm present) untouched', () =>
  withTempHayateRoot(async (hayateRoot) => {
    const manifest = fakeManifest([fakeTarget('pkg')]);
    const dir = join(hayateRoot, 'wasm-pkgs/pkg');
    await mkdir(dir, { recursive: true });
    await writeFile(join(dir, 'hayate_adapter_web_bg.wasm'), 'not really wasm, just a marker');
    await writeFile(join(dir, 'package.json'), 'real build package.json, not the stub');

    await bootstrapWasmPkgs({ hayateRoot, manifest });

    assert.equal(await readFile(join(dir, 'package.json'), 'utf8'), 'real build package.json, not the stub');
  }));

// The whole point of #702: a new backend needs one manifest entry, not a
// matching new stub-dir literal in this script.
test('a brand new manifest entry gets a stub with no special-casing', () =>
  withTempHayateRoot(async (hayateRoot) => {
    const manifest = fakeManifest([fakeTarget('pkg-quantum')]);

    await bootstrapWasmPkgs({ hayateRoot, manifest });

    const pkgJson = JSON.parse(await readFile(join(hayateRoot, 'wasm-pkgs/pkg-quantum/package.json'), 'utf8'));
    assert.equal(pkgJson.name, 'hayate-adapter-web-pkg-quantum');
  }));
