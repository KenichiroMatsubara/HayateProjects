// wasm-manifest.mjs — pure, testable core for the wasm build manifest.
//
// wasm-build-manifest.json is the single source of truth for which
// hayate-adapter-web wasm-pkgs/* builds exist. Scripts that need that list
// (build-wasm.mjs, clean-wasm-pkgs.mjs, ...) read it through this module
// instead of hardcoding backend names/dirs/cargo flags themselves. Kept free
// of process/spawn concerns so it can be unit-tested without invoking cargo.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const MANIFEST_PATH = join(dirname(fileURLToPath(import.meta.url)), 'wasm-build-manifest.json');

const FEATURE_MODES = new Set(['inherit', 'exclusive', 'additive']);

function invalid(target, message) {
  const where = target ? `target "${target.name ?? JSON.stringify(target)}"` : 'manifest';
  throw new Error(`invalid wasm build manifest: ${where}: ${message}`);
}

function validateTarget(target) {
  if (typeof target.name !== 'string' || target.name === '') invalid(target, 'name must be a non-empty string');
  if (typeof target.outDir !== 'string') invalid(target, 'outDir must be a string');
  if (typeof target.targetDir !== 'string') invalid(target, 'targetDir must be a string');
  if (typeof target.description !== 'string') invalid(target, 'description must be a string');
  if (typeof target.includeInDefaultBuild !== 'boolean') invalid(target, 'includeInDefaultBuild must be a boolean');
  if (typeof target.npmName !== 'string' || target.npmName === '') invalid(target, 'npmName must be a non-empty string');

  // host maps a target to the CanvasBackend value Hayate/host/src/index.ts's
  // loadCanvasBackend loads it for (null when nothing in the host imports this
  // target, e.g. pkg-null is Tsubame-test-only — see #703).
  if (target.host !== null) {
    if (typeof target.host !== 'object') invalid(target, 'host must be null or an object');
    if (typeof target.host.backend !== 'string') invalid(target, 'host.backend must be a string');
  }

  const { cargoFeatures } = target;
  if (!cargoFeatures || !FEATURE_MODES.has(cargoFeatures.mode)) {
    invalid(target, `cargoFeatures.mode must be one of ${[...FEATURE_MODES].join(', ')}`);
  }
  if (!Array.isArray(cargoFeatures.names)) invalid(target, 'cargoFeatures.names must be an array');
  if (cargoFeatures.mode !== 'inherit' && cargoFeatures.names.length === 0) {
    invalid(target, `cargoFeatures.names must be non-empty for mode "${cargoFeatures.mode}"`);
  }
}

// Exported separately from loadManifest so tests can validate hand-built
// manifest objects without touching the filesystem.
export function validateManifest(manifest) {
  if (typeof manifest.crateDir !== 'string') invalid(null, 'crateDir must be a string');
  if (typeof manifest.npmPackageName !== 'string') invalid(null, 'npmPackageName must be a string');
  if (!Array.isArray(manifest.targets) || manifest.targets.length === 0) {
    invalid(null, 'targets must be a non-empty array');
  }
  manifest.targets.forEach(validateTarget);

  const names = manifest.targets.map((t) => t.name);
  if (new Set(names).size !== names.length) {
    invalid(null, `target names must be unique (got: ${names.join(', ')})`);
  }
  return manifest;
}

export function loadManifest(path = MANIFEST_PATH) {
  return validateManifest(JSON.parse(readFileSync(path, 'utf8')));
}

// No names => every target marked includeInDefaultBuild (today's `pnpm run
// build`). Explicit names => exactly those targets, in the given order,
// regardless of includeInDefaultBuild (today's `pnpm run build:layer-present`,
// generalized to any target by name). { all: true } => literally every target
// regardless of includeInDefaultBuild — the seam CI's Pages deploy uses so a
// new manifest entry can't be silently left out of the deployed artifact.
export function selectTargets(manifest, names, { all = false } = {}) {
  if (all) return manifest.targets;
  if (names.length === 0) return manifest.targets.filter((t) => t.includeInDefaultBuild);
  return names.map((name) => {
    const target = manifest.targets.find((t) => t.name === name);
    if (!target) {
      const known = manifest.targets.map((t) => t.name).join(', ');
      throw new Error(`unknown wasm build target "${name}" (known targets: ${known})`);
    }
    return target;
  });
}

// The 3 cargo feature compositions the existing backends need: build with the
// crate's Cargo.toml default features untouched ("inherit"), replace them
// entirely with an explicit set ("exclusive"), or layer an explicit set on top
// of the defaults ("additive" — this is how layer-present builds today).
export function cargoArgsFor(target) {
  const { mode, names } = target.cargoFeatures;
  if (mode === 'inherit') return [];
  const features = ['--features', names.join(',')];
  return mode === 'exclusive' ? ['--no-default-features', ...features] : features;
}

export function wasmPackArgsFor(target, crateDir, outDir) {
  const base = ['build', crateDir, '--target', 'web', '--out-dir', outDir];
  const extra = cargoArgsFor(target);
  return extra.length > 0 ? [...base, '--', ...extra] : base;
}

export function outDirFor(target, rootDir) {
  return join(rootDir, target.outDir);
}

export function targetDirFor(target, rootDir) {
  return join(rootDir, target.targetDir);
}

export const GITIGNORE_CONTENTS = '*\n!package.json\n';

// wasm-pack regenerates package.json on every build but its shape drifts
// across wasm-pack versions (0.15 drops description/repository/files/
// sideEffects), which used to create noisy diffs in the tracked file. Pin a
// canonical version here so there is exactly one definition of "the
// package.json for a wasm-pkgs dir" instead of one per build script.
// `sideEffects` must stay — it's required for bundlers to not tree-shake
// wasm-bindgen's snippets.
//
// The package name is the target's own npmName, not the shared crate-level
// npmPackageName (#765). host depends on pkg / pkg-tiny-skia / pkg-vello-cpu as
// three sibling file: deps under three alias keys
// (hayate-adapter-web[-cpu|-vello-cpu]). When all three package.jsons declared
// the same name "hayate-adapter-web", pnpm hit a name collision and routed one
// alias through a .pnpm virtual-store copy that only carried package.json (no
// .js), so Rolldown failed to resolve the dynamic import('hayate-adapter-web-cpu')
// in the Pages demo build. Naming each dir after its npmName removes the
// collision so every alias links straight to its source dir.
export function packageJsonFor(target, manifest) {
  void manifest;
  return `${JSON.stringify(
    {
      name: target.npmName,
      type: 'module',
      description: target.description,
      version: '0.1.0',
      license: 'Apache-2.0',
      repository: {
        type: 'git',
        url: 'https://github.com/KenichiroMatsubara/HayateProjects',
      },
      files: ['hayate_adapter_web_bg.wasm', 'hayate_adapter_web.js', 'hayate_adapter_web.d.ts'],
      main: 'hayate_adapter_web.js',
      types: 'hayate_adapter_web.d.ts',
      sideEffects: ['./snippets/*'],
    },
    null,
    2,
  )}\n`;
}
