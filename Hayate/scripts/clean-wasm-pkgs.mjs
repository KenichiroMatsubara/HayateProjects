#!/usr/bin/env node
// scripts/clean-wasm-pkgs.mjs — remove every wasm-pkgs/* output dir declared in
// wasm-build-manifest.json (#700). Replaces a hand-maintained `rimraf` dir list
// in Hayate/package.json's `clean` script, which had silently drifted out of
// sync with the real set of backends (missing pkg-vello-cpu and
// pkg-layer-present) — reading the manifest instead means `clean` can't drift
// again when a target is added or renamed.
import { rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { loadManifest, outDirFor } from './wasm-manifest.mjs';

const ROOT_DIR = join(dirname(fileURLToPath(import.meta.url)), '..');
const manifest = loadManifest();

await Promise.all(manifest.targets.map((target) => rm(outDirFor(target, ROOT_DIR), { recursive: true, force: true })));
