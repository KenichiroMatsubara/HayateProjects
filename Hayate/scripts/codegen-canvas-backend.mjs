#!/usr/bin/env node
// scripts/codegen-canvas-backend.mjs — regenerate
// Hayate/host/src/load-canvas-backend.generated.ts from wasm-build-manifest.json
// (#700/#703). Run after editing the manifest's `host` mappings; `npm run
// check:canvas-backend` fails CI if someone forgets to.
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { generateLoadCanvasBackend } from './codegen-canvas-backend.lib.mjs';
import { loadManifest } from './wasm-manifest.mjs';

const ROOT_DIR = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT_FILE = join(ROOT_DIR, 'host', 'src', 'load-canvas-backend.generated.ts');

writeFileSync(OUT_FILE, generateLoadCanvasBackend(loadManifest()));
console.log(`wrote ${OUT_FILE}`);
