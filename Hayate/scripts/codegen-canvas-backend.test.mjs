import { test } from 'node:test';
import assert from 'node:assert/strict';

import { generateLoadCanvasBackend } from './codegen-canvas-backend.lib.mjs';
import { loadManifest } from './wasm-manifest.mjs';

test('generates a static import and a one-argument init for each backend', () => {
  const manifest = {
    targets: [
      { name: 'pkg-tiny-skia', npmName: 'hayate-adapter-web-cpu', host: { backend: 'tiny-skia' } },
      { name: 'pkg-null', npmName: 'hayate-adapter-web-null', host: null },
    ],
  };
  const source = generateLoadCanvasBackend(manifest);
  assert.match(source, /if \(backend === 'tiny-skia'\)/);
  assert.match(source, /await import\('hayate-adapter-web-cpu'\)/);
  assert.match(source, /HayateElementRenderer\.init\(canvas\)/);
  assert.doesNotMatch(source, /hayate-adapter-web-null/);
  assert.doesNotMatch(source, /layerPresent|cpuLayerPresent/);
});

test('a backend with more than one target throws a clear error', () => {
  const manifest = {
    targets: [
      { name: 'pkg', npmName: 'hayate-adapter-web', host: { backend: 'vello' } },
      { name: 'pkg-other', npmName: 'hayate-adapter-web-other', host: { backend: 'vello' } },
    ],
  };
  assert.throws(() => generateLoadCanvasBackend(manifest), /vello.*more than one target/);
});

test('every generated import is a static string literal', () => {
  const source = generateLoadCanvasBackend(loadManifest());
  const importCalls = [...source.matchAll(/import\(([^)]*)\)/g)].map((match) => match[1]);
  assert.ok(importCalls.length > 0);
  for (const argument of importCalls) {
    assert.match(argument.trim(), /^'[^']+'$/);
  }
});

test('the real manifest generates both production branches with no escape hatch', () => {
  const source = generateLoadCanvasBackend(loadManifest());
  assert.match(source, /if \(backend === 'vello'\)/);
  assert.match(source, /await import\('@torimi\/hayate-adapter-web'\)/);
  assert.match(source, /if \(backend === 'tiny-skia'\)/);
  assert.match(source, /await import\('@torimi\/hayate-adapter-web-cpu'\)/);
  assert.equal((source.match(/HayateElementRenderer\.init\(canvas\)/g) ?? []).length, 2);
  assert.doesNotMatch(source, /layerPresent|cpuLayerPresent|hayate-adapter-web-layer-present/);
  assert.match(source, /AUTO-GENERATED/);
});
