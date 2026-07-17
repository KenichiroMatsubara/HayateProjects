// bootstrap-wasm-pkgs.lib.mjs — pure-ish, testable core for the stub generator.
//
// Which dirs need a stub is read from Hayate/scripts/wasm-build-manifest.json
// (#700) via wasm-manifest.mjs's outDirFor, instead of a hand-maintained list
// here — the same manifest entry that makes a target buildable also makes it
// bootstrap-able, with nothing to keep in sync by hand.
import { access, mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { GITIGNORE_CONTENTS, loadManifest, outDirFor, readmeFor } from '../Hayate/scripts/wasm-manifest.mjs';

// The stub package name is the target's own npmName, not the shared crate name
// (#765). host imports pkg / pkg-tiny-skia as sibling
// file: deps under distinct alias keys; if every stub declared the same name
// "hayate-adapter-web", pnpm collided on the name at install time and routed one
// alias through a .pnpm virtual-store copy holding only package.json (no .js),
// which broke Rolldown's dynamic-import resolution in the Pages demo build. This
// stub is written at preinstall — before any wasm build — so it, not just the
// post-build package.json, has to carry the distinct name.
export function packageJsonStub(target) {
  // Mirror the publish split that packageJsonFor bakes into the built
  // package.json (public → publishConfig.access, private → private), so a
  // `pnpm -r publish --dry-run` run against a fresh clone (stubs, no wasm yet)
  // sees the same public/private closure the real release build would.
  const publishField = target.private ? '  "private": true,' : '  "publishConfig": { "access": "public" },';
  return `{
  "name": "${target.npmName}",
  "type": "module",
  "version": "0.1.0",
  "license": "Apache-2.0",
${publishField}
  "main": "hayate_adapter_web.js",
  "types": "hayate_adapter_web.d.ts"
}
`;
}

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
    await writeFile(join(dir, 'package.json'), packageJsonStub(target));
    await writeFile(join(dir, 'hayate_adapter_web.js'), JS_STUB);
    await writeFile(join(dir, 'hayate_adapter_web.d.ts'), DTS_STUB);
    await writeFile(join(dir, '.gitignore'), GITIGNORE_CONTENTS);
    // README is committed (the .gitignore whitelists it) so a fresh clone always
    // has it; write it here too so a brand-new backend dir gets one (#773).
    await writeFile(join(dir, 'README.md'), readmeFor(target));
  }
}
