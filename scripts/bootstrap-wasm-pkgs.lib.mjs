// bootstrap-wasm-pkgs.lib.mjs — pure-ish, testable core for the stub generator.
//
// Which dirs need a stub is read from Hayate/scripts/wasm-build-manifest.json
// (#700) via wasm-manifest.mjs's outDirFor, instead of a hand-maintained list
// here — the same manifest entry that makes a target buildable also makes it
// bootstrap-able, with nothing to keep in sync by hand.
import { access, mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { GITIGNORE_CONTENTS, loadManifest, outDirFor } from '../Hayate/scripts/wasm-manifest.mjs';

export const PACKAGE_JSON = `{
  "name": "hayate-adapter-web",
  "type": "module",
  "version": "0.1.0",
  "license": "Apache-2.0",
  "main": "hayate_adapter_web.js",
  "types": "hayate_adapter_web.d.ts"
}
`;

export const JS_STUB = `export default async function init() {}
export class HayateElementRenderer {
  static async init() {
    throw new Error('WASM not built — run: pnpm --filter hayate build');
  }
}
export class HayateElementHtmlRenderer {}
`;

export const DTS_STUB = `export default function init(): Promise<void>;
export class HayateElementRenderer {
  static init(canvas: HTMLCanvasElement, layerPresentEnabled?: boolean): Promise<unknown>;
}
export class HayateElementHtmlRenderer {}
`;

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

// hayateRoot is Hayate/'s own root (not the repo root) — the same directory
// build-wasm.mjs calls ROOT_DIR, since manifest outDir entries are relative
// to it (e.g. "wasm-pkgs/pkg").
export async function bootstrapWasmPkgs({ hayateRoot, manifest = loadManifest() }) {
  for (const target of manifest.targets) {
    const dir = outDirFor(target, hayateRoot);
    if (await exists(join(dir, 'hayate_adapter_web_bg.wasm'))) continue;

    await mkdir(dir, { recursive: true });
    await writeFile(join(dir, 'package.json'), PACKAGE_JSON);
    await writeFile(join(dir, 'hayate_adapter_web.js'), JS_STUB);
    await writeFile(join(dir, 'hayate_adapter_web.d.ts'), DTS_STUB);
    await writeFile(join(dir, '.gitignore'), GITIGNORE_CONTENTS);
  }
}
