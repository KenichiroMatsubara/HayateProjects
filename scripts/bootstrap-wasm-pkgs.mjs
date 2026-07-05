#!/usr/bin/env node
/**
 * Create stub hayate-adapter-web packages when WASM artifacts are missing.
 * Allows pnpm install on a fresh clone before `pnpm --filter hayate build`.
 *
 * Which dirs get a stub is read from Hayate/scripts/wasm-build-manifest.json
 * (#700) — see bootstrap-wasm-pkgs.lib.mjs.
 */
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { bootstrapWasmPkgs } from './bootstrap-wasm-pkgs.lib.mjs';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

await bootstrapWasmPkgs({ hayateRoot: join(repoRoot, 'Hayate') });
