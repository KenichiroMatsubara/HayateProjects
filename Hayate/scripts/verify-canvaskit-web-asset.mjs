import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { basename, resolve } from 'node:path';

const distDir = resolve(process.argv[2] ?? 'dist');
const base = `${process.env.VITE_BASE ?? '/'}`.replace(/\/?$/, '/');

async function filesBelow(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  return (await Promise.all(entries.map(async (entry) => {
    const path = resolve(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  }))).flat();
}

const files = await filesBelow(distDir);
const canvasKitWasm = files.filter((file) => /^canvaskit-[^.]+\.wasm$/.test(basename(file)));

assert.equal(
  canvasKitWasm.length,
  1,
  `expected one emitted CanvasKit WASM asset in ${distDir}, found ${canvasKitWasm.length}`,
);

const wasmName = basename(canvasKitWasm[0]);
const javascript = await Promise.all(
  files.filter((file) => file.endsWith('.js')).map((file) => readFile(file, 'utf8')),
);
const expectedPublicUrl = `${base}assets/${wasmName}`;

assert.ok(
  javascript.some((source) => source.includes(expectedPublicUrl)),
  `expected emitted JavaScript to reference ${expectedPublicUrl}`,
);
assert.ok(
  javascript.every((source) => !source.includes(`${wasmName}?url`)),
  `expected emitted JavaScript not to retain the unresolved ${wasmName}?url path`,
);

console.log(`verified CanvasKit web asset: ${expectedPublicUrl}`);
