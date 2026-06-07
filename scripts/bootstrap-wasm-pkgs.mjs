#!/usr/bin/env node
/**
 * Create stub hayate-adapter-web packages when WASM artifacts are missing.
 * Allows pnpm install on a fresh clone before `pnpm run build:wasm`.
 */
import { access, mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const PKG_DIRS = [
  join(root, 'Hayate/examples/web-demo/pkg'),
  join(root, 'Hayate/examples/web-demo/pkg-tiny-skia'),
  join(root, 'Hayate/examples/web-demo/pkg-null'),
];

const PACKAGE_JSON = `{
  "name": "hayate-adapter-web",
  "type": "module",
  "version": "0.1.0",
  "license": "Apache-2.0",
  "main": "hayate_adapter_web.js",
  "types": "hayate_adapter_web.d.ts"
}
`;

const JS_STUB = `export default async function init() {}
export class HayateElementRenderer {
  static async init() {
    throw new Error('WASM not built — run: pnpm run build:wasm');
  }
}
export class HayateElementHtmlRenderer {}
`;

const DTS_STUB = `export default function init(): Promise<void>;
export class HayateElementRenderer {
  static init(canvas: HTMLCanvasElement): Promise<unknown>;
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

for (const dir of PKG_DIRS) {
  const wasm = join(dir, 'hayate_adapter_web_bg.wasm');
  if (await exists(wasm)) continue;

  await mkdir(dir, { recursive: true });
  await writeFile(join(dir, 'package.json'), PACKAGE_JSON);
  await writeFile(join(dir, 'hayate_adapter_web.js'), JS_STUB);
  await writeFile(join(dir, 'hayate_adapter_web.d.ts'), DTS_STUB);
  await writeFile(join(dir, '.gitignore'), '*\n!package.json\n');
}
