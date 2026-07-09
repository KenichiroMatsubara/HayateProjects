// wasm-manifest.test.mjs — unit tests for the wasm build manifest core.
// Run with: node --test Hayate/scripts/
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { join } from 'node:path';

import {
  loadManifest,
  wasmPackArgsFor,
  outDirFor,
  targetDirFor,
  packageJsonFor,
  GITIGNORE_CONTENTS,
  selectTargets,
  validateManifest,
  cargoArgsFor,
} from './wasm-manifest.mjs';

test('the manifest declares exactly the 4 known wasm-pkgs targets', () => {
  const manifest = loadManifest();
  const names = manifest.targets.map((t) => t.name);

  assert.deepEqual(names, ['pkg', 'pkg-tiny-skia', 'pkg-vello-cpu', 'pkg-null']);
});

// Pins the exact `wasm-pack build` argv the two legacy scripts used to hardcode,
// so consolidating them into one manifest-driven script can't silently change
// which cargo features get built for any of the 5 existing pkgs.
test('wasmPackArgsFor reproduces each legacy script\'s exact argv', () => {
  const manifest = loadManifest();
  const byName = Object.fromEntries(manifest.targets.map((t) => [t.name, t]));
  const crateDir = 'crates/platform/web';

  assert.deepEqual(wasmPackArgsFor(byName['pkg'], crateDir, 'wasm-pkgs/pkg'), [
    'build',
    crateDir,
    '--target',
    'web',
    '--out-dir',
    'wasm-pkgs/pkg',
  ]);

  assert.deepEqual(wasmPackArgsFor(byName['pkg-tiny-skia'], crateDir, 'wasm-pkgs/pkg-tiny-skia'), [
    'build',
    crateDir,
    '--target',
    'web',
    '--out-dir',
    'wasm-pkgs/pkg-tiny-skia',
    '--',
    '--no-default-features',
    '--features',
    'backend-tiny-skia',
  ]);

  assert.deepEqual(wasmPackArgsFor(byName['pkg-vello-cpu'], crateDir, 'wasm-pkgs/pkg-vello-cpu'), [
    'build',
    crateDir,
    '--target',
    'web',
    '--out-dir',
    'wasm-pkgs/pkg-vello-cpu',
    '--',
    '--no-default-features',
    '--features',
    'backend-vello-cpu',
  ]);

  assert.deepEqual(wasmPackArgsFor(byName['pkg-null'], crateDir, 'wasm-pkgs/pkg-null'), [
    'build',
    crateDir,
    '--target',
    'web',
    '--out-dir',
    'wasm-pkgs/pkg-null',
    '--',
    '--no-default-features',
    '--features',
    'backend-null',
  ]);
});

// Pins the exact OUT_DIR*/TARGET_DIR* constants the legacy scripts hardcoded,
// so per-backend CARGO_TARGET_DIR isolation (the whole point of those consts —
// see the comment in build-wasm.mjs about the "feature tug-of-war") survives
// the move to a manifest.
test('outDirFor and targetDirFor reproduce the legacy OUT_DIR*/TARGET_DIR* paths', () => {
  const manifest = loadManifest();
  const byName = Object.fromEntries(manifest.targets.map((t) => [t.name, t]));
  const root = '/repo/Hayate';

  const expected = {
    pkg: ['wasm-pkgs/pkg', 'target/wasm'],
    'pkg-tiny-skia': ['wasm-pkgs/pkg-tiny-skia', 'target/wasm-tiny-skia'],
    'pkg-vello-cpu': ['wasm-pkgs/pkg-vello-cpu', 'target/wasm-vello-cpu'],
    'pkg-null': ['wasm-pkgs/pkg-null', 'target/wasm-null'],
  };

  for (const [name, [outDir, targetDir]] of Object.entries(expected)) {
    assert.equal(outDirFor(byName[name], root), join(root, outDir));
    assert.equal(targetDirFor(byName[name], root), join(root, targetDir));
  }
});

// wasm-pack regenerates package.json on every build but its shape drifts
// across wasm-pack versions (0.15 drops description/repository/files/
// sideEffects) — the legacy scripts pinned a canonical version to stop that
// from creating noisy diffs in the tracked file. Pin the exact same shape here.
test('packageJsonFor reproduces the legacy canonical package.json, per-target description', () => {
  const manifest = loadManifest();
  const byName = Object.fromEntries(manifest.targets.map((t) => [t.name, t]));

  const pkgJson = JSON.parse(packageJsonFor(byName['pkg'], manifest));
  assert.deepEqual(pkgJson, {
    name: '@hayate/adapter-web',
    type: 'module',
    description: 'Hayate — GPU-native UI substrate',
    version: '0.1.0',
    license: 'Apache-2.0',
    repository: { type: 'git', url: 'https://github.com/KenichiroMatsubara/HayateProjects' },
    publishConfig: { access: 'public' },
    scripts: {
      prepublishOnly:
        "node -e \"process.env.GITHUB_ACTIONS||(console.error('publish only via the release workflow (ADR-0007 §4)'),process.exit(1))\"",
    },
    files: ['hayate_adapter_web_bg.wasm', 'hayate_adapter_web.js', 'hayate_adapter_web.d.ts'],
    main: 'hayate_adapter_web.js',
    types: 'hayate_adapter_web.d.ts',
    sideEffects: ['./snippets/*'],
  });
  // pkg-null is the test-only null backend — kept out of the public npm
  // closure via `private`, never `publishConfig.access` (ADR-0007 §1).
  assert.equal(JSON.parse(packageJsonFor(byName['pkg-null'], manifest)).private, true);
  assert.equal(JSON.parse(packageJsonFor(byName['pkg-null'], manifest)).publishConfig, undefined);
  // Each pkg dir's package name is its own npmName, NOT the shared crate name
  // (#765). When several sibling file: deps (host imports pkg / pkg-tiny-skia /
  // pkg-vello-cpu) all declared name "hayate-adapter-web", pnpm saw a name
  // collision and routed one alias (hayate-adapter-web-cpu) through a
  // .pnpm virtual-store copy that only carried package.json — Rolldown then
  // failed to resolve the dynamic import in the Pages demo build. Distinct
  // names make every alias link straight to its source dir.
  assert.equal(JSON.parse(packageJsonFor(byName['pkg-tiny-skia'], manifest)).name, '@hayate/adapter-web-cpu');
  assert.equal(JSON.parse(packageJsonFor(byName['pkg-vello-cpu'], manifest)).name, '@hayate/adapter-web-vello-cpu');
  assert.equal(JSON.parse(packageJsonFor(byName['pkg-null'], manifest)).name, 'hayate-adapter-web-null');

  assert.equal(GITIGNORE_CONTENTS, '*\n!package.json\n');
});

// No args = today's `pnpm run build` (the 4 non-layer-present backends);
// an explicit name = today's `pnpm run build:layer-present` (one target only).
// This is what lets the two legacy scripts collapse into one CLI.
test('selectTargets: no names selects the default-build set, in manifest order', () => {
  const manifest = loadManifest();
  const selected = selectTargets(manifest, []);

  assert.deepEqual(
    selected.map((t) => t.name),
    ['pkg', 'pkg-tiny-skia', 'pkg-vello-cpu', 'pkg-null'],
  );
});

test('selectTargets: an explicit name selects just that target, default or not', () => {
  const manifest = loadManifest();

  assert.deepEqual(selectTargets(manifest, ['pkg-null']).map((t) => t.name), ['pkg-null']);
  assert.deepEqual(selectTargets(manifest, ['pkg-null', 'pkg']).map((t) => t.name), ['pkg-null', 'pkg']);
});

test('selectTargets: an unknown name throws a clear error', () => {
  const manifest = loadManifest();

  assert.throws(() => selectTargets(manifest, ['pkg-quantum']), /pkg-quantum/);
});

// CI's Pages deploy needs every target built, not just the default set — { all: true } is the
// seam that guarantees a manifest entry can never be silently missing from that build,
// regardless of its includeInDefaultBuild value. Since ADR-0140 (#718) removed the manifest's
// one opt-in target (pkg-layer-present), the current manifest happens to have { all: true }
// equal the default-build set — the seam itself still exists for the next opt-in target.
test('selectTargets: { all: true } selects every target, default or opt-in', () => {
  const manifest = loadManifest();

  assert.deepEqual(selectTargets(manifest, [], { all: true }).map((t) => t.name), [
    'pkg',
    'pkg-tiny-skia',
    'pkg-vello-cpu',
    'pkg-null',
  ]);
});

// Pins the exact npmName/host mapping loadCanvasBackend's codegen depends on (#703) —
// including the real naming mismatch (pkg-tiny-skia's bare specifier is "-cpu", not
// "-tiny-skia"), pkg-null having no host consumer, and each per-layer-present opt-in
// target naming its own init() runtime arg (ADR-0138/ADR-0140, #710/#717/#718).
test('npmName/host mapping matches what Hayate/host/src actually imports', () => {
  const manifest = loadManifest();
  const byName = Object.fromEntries(manifest.targets.map((t) => [t.name, t]));

  assert.equal(byName['pkg'].npmName, '@hayate/adapter-web');
  assert.deepEqual(byName['pkg'].host, { backend: 'vello', runtimeLayerPresentArg: 'layerPresent' });

  assert.equal(byName['pkg-tiny-skia'].npmName, '@hayate/adapter-web-cpu');
  assert.deepEqual(byName['pkg-tiny-skia'].host, { backend: 'tiny-skia', runtimeLayerPresentArg: 'cpuLayerPresent' });

  assert.equal(byName['pkg-vello-cpu'].npmName, '@hayate/adapter-web-vello-cpu');
  assert.deepEqual(byName['pkg-vello-cpu'].host, { backend: 'vello-cpu', runtimeLayerPresentArg: 'cpuLayerPresent' });

  assert.equal(byName['pkg-null'].npmName, 'hayate-adapter-web-null');
  assert.equal(byName['pkg-null'].host, null);
});

function validManifestFixture(overrides = {}) {
  return {
    crateDir: 'crates/platform/web',
    npmPackageName: 'hayate-adapter-web',
    targets: [
      {
        name: 'pkg',
        outDir: 'wasm-pkgs/pkg',
        targetDir: 'target/wasm',
        cargoFeatures: { mode: 'inherit', names: [] },
        description: 'x',
        includeInDefaultBuild: true,
        npmName: 'hayate-adapter-web',
        host: { backend: 'vello' },
      },
    ],
    ...overrides,
  };
}

test('validateManifest rejects an exclusive-mode target with no feature names', () => {
  const manifest = validManifestFixture({
    targets: [
      {
        name: 'pkg-broken',
        outDir: 'wasm-pkgs/pkg-broken',
        targetDir: 'target/wasm-broken',
        cargoFeatures: { mode: 'exclusive', names: [] },
        description: 'x',
        includeInDefaultBuild: true,
        npmName: 'hayate-adapter-web-broken',
        host: null,
      },
    ],
  });

  assert.throws(() => validateManifest(manifest), /pkg-broken.*cargoFeatures\.names/);
});

test('validateManifest rejects a target missing npmName', () => {
  const target = { ...validManifestFixture().targets[0] };
  delete target.npmName;
  const manifest = validManifestFixture({ targets: [target] });

  assert.throws(() => validateManifest(manifest), /npmName/);
});

test('validateManifest accepts host: null (no host-side consumer, e.g. pkg-null)', () => {
  const manifest = validManifestFixture({ targets: [{ ...validManifestFixture().targets[0], host: null }] });

  assert.doesNotThrow(() => validateManifest(manifest));
});

test('validateManifest rejects a host object missing backend', () => {
  const manifest = validManifestFixture({
    targets: [{ ...validManifestFixture().targets[0], host: { runtimeLayerPresentArg: 'layerPresent' } }],
  });

  assert.throws(() => validateManifest(manifest), /host\.backend/);
});

test('validateManifest rejects duplicate target names', () => {
  const target = validManifestFixture().targets[0];
  const manifest = validManifestFixture({ targets: [target, { ...target }] });

  assert.throws(() => validateManifest(manifest), /unique/);
});

// The whole point of the manifest: a future backend should need ONE new JSON
// entry, not a matching new branch in every consuming script. Prove it here
// with a hypothetical 6th target that no script has ever seen by name.
test('a brand new manifest entry needs no special-casing in any helper', () => {
  const manifest = validManifestFixture({
    targets: [
      ...validManifestFixture().targets,
      {
        name: 'pkg-quantum',
        outDir: 'wasm-pkgs/pkg-quantum',
        targetDir: 'target/wasm-quantum',
        cargoFeatures: { mode: 'exclusive', names: ['backend-quantum'] },
        description: 'Hayate — quantum backend (hypothetical)',
        includeInDefaultBuild: false,
        npmName: 'hayate-adapter-web-quantum',
        host: null,
      },
    ],
  });
  validateManifest(manifest); // does not throw
  const target = manifest.targets.find((t) => t.name === 'pkg-quantum');

  assert.deepEqual(cargoArgsFor(target), ['--no-default-features', '--features', 'backend-quantum']);
  assert.equal(outDirFor(target, '/root'), '/root/wasm-pkgs/pkg-quantum');
  assert.equal(targetDirFor(target, '/root'), '/root/target/wasm-quantum');
  assert.equal(JSON.parse(packageJsonFor(target, manifest)).description, 'Hayate — quantum backend (hypothetical)');
  assert.deepEqual(selectTargets(manifest, ['pkg-quantum']).map((t) => t.name), ['pkg-quantum']);
  assert.ok(!selectTargets(manifest, []).some((t) => t.name === 'pkg-quantum'), 'opt-in target stays out of the default build set');
});
