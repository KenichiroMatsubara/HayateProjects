// codegen-canvas-backend.test.mjs — unit tests for the loadCanvasBackend codegen.
// Run with: node --test Hayate/scripts/
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { generateLoadCanvasBackend } from './codegen-canvas-backend.lib.mjs';
import { loadManifest } from './wasm-manifest.mjs';

test('generates a static import() branch for a plain (non-variant) backend', () => {
  const manifest = {
    targets: [
      { name: 'pkg-tiny-skia', npmName: 'hayate-adapter-web-cpu', host: { backend: 'tiny-skia' } },
      { name: 'pkg-null', npmName: 'hayate-adapter-web-null', host: null },
    ],
  };

  const source = generateLoadCanvasBackend(manifest);

  assert.match(source, /if \(backend === 'tiny-skia'\)/);
  assert.match(source, /await import\('hayate-adapter-web-cpu'\)/);
  // pkg-null has no host entry — it must not appear in the generated branches at all.
  assert.doesNotMatch(source, /hayate-adapter-web-null/);
});

// ADR-0140 (#718): vello's layer-present is now a runtime toggle threaded into init(),
// same mechanism as tiny-skia/vello_cpu's runtimeLayerPresentArg — not a second
// compile-time package variant. `layerPresent` stays its own base parameter (not
// duplicated into the runtime-args list) since it's always present in the signature.
test('threads vello\'s own layerPresent runtime arg into init(), without duplicating the base param', () => {
  const manifest = {
    targets: [{ name: 'pkg', npmName: 'hayate-adapter-web', host: { backend: 'vello', runtimeLayerPresentArg: 'layerPresent' } }],
  };

  const source = generateLoadCanvasBackend(manifest);

  assert.match(source, /if \(backend === 'vello'\)/);
  assert.match(source, /await import\('hayate-adapter-web'\)/);
  assert.match(source, /await mod\.HayateElementRenderer\.init\(canvas, layerPresent\)/);
  // exactly one `layerPresent = true,` param — not duplicated by the runtime-args list.
  assert.equal((source.match(/layerPresent = true,/g) ?? []).length, 1);
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

// ADR-0138 (#710): tiny-skia/vello_cpu take a runtime per-layer-present toggle
// distinct from vello's compile-time `layerPresent` package variant. A target
// opts in via `host.runtimeLayerPresentArg`, which threads a second `init()`
// argument and adds `cpuLayerPresent` to loadCanvasBackend's own signature.
test('threads a runtime layer-present arg into init() for a backend that opts in', () => {
  const manifest = {
    targets: [
      {
        name: 'pkg-tiny-skia',
        npmName: 'hayate-adapter-web-cpu',
        host: { backend: 'tiny-skia', runtimeLayerPresentArg: 'cpuLayerPresent' },
      },
    ],
  };

  const source = generateLoadCanvasBackend(manifest);

  assert.match(source, /cpuLayerPresent = true/);
  assert.match(source, /await mod\.HayateElementRenderer\.init\(canvas, cpuLayerPresent\)/);
});

// #717 (prefactor for #718): runtimeLayerPresentArg is the literal init() variable
// name a target opts in with, not just an on/off bool — a backend other than
// tiny-skia/vello-cpu must be able to thread its own differently-named flag.
test('threads a target-chosen variable name (not hardcoded to cpuLayerPresent) into init()', () => {
  const manifest = {
    targets: [
      {
        name: 'pkg-tiny-skia',
        npmName: 'hayate-adapter-web-cpu',
        host: { backend: 'tiny-skia', runtimeLayerPresentArg: 'fooFlag' },
      },
    ],
  };

  const source = generateLoadCanvasBackend(manifest);

  assert.match(source, /fooFlag = true/);
  assert.match(source, /await mod\.HayateElementRenderer\.init\(canvas, fooFlag\)/);
  assert.doesNotMatch(source, /cpuLayerPresent = true/);
  assert.doesNotMatch(source, /init\(canvas, cpuLayerPresent\)/);
});

test('leaves init() at a single canvas arg for a backend that does not opt in', () => {
  const manifest = {
    targets: [{ name: 'pkg-tiny-skia', npmName: 'hayate-adapter-web-cpu', host: { backend: 'tiny-skia' } }],
  };

  const source = generateLoadCanvasBackend(manifest);

  assert.doesNotMatch(source, /cpuLayerPresent/);
  assert.match(source, /await mod\.HayateElementRenderer\.init\(canvas\)/);
});

// The whole point of #703: every import() must stay a literal string a bundler
// can statically analyze — never a computed/dynamic specifier.
test('every import() call in the generated source is a static string literal', () => {
  const manifest = loadManifest();
  const source = generateLoadCanvasBackend(manifest);

  const importCalls = [...source.matchAll(/import\(([^)]*)\)/g)].map((m) => m[1]);
  assert.ok(importCalls.length > 0, 'expected at least one import() call');
  for (const arg of importCalls) {
    assert.match(arg.trim(), /^'[^']+'$/, `import() arg "${arg}" is not a static single-quoted string literal`);
  }
});

// Regenerating from the real manifest must reproduce the exact branch shape
// Hayate/host/src/index.ts hand-wrote before #703 — same 3 CanvasBackend
// branches, same bare specifiers. Since ADR-0140 (#718), vello no longer has a
// separate layer-present package — it always imports hayate-adapter-web and
// threads `layerPresent` into init() the same way tiny-skia/vello_cpu thread
// `cpuLayerPresent`.
test('the real manifest reproduces the original hand-written loadCanvasBackend branches', () => {
  const manifest = loadManifest();
  const source = generateLoadCanvasBackend(manifest);

  assert.match(source, /if \(backend === 'vello'\)/);
  assert.match(source, /await import\('hayate-adapter-web'\)/);
  assert.match(source, /await mod\.HayateElementRenderer\.init\(canvas, layerPresent\)/);
  assert.match(source, /if \(backend === 'tiny-skia'\)/);
  assert.match(source, /await import\('hayate-adapter-web-cpu'\)/);
  assert.match(source, /if \(backend === 'vello-cpu'\)/);
  assert.match(source, /await import\('hayate-adapter-web-vello-cpu'\)/);
  // ADR-0138 (#710): tiny-skia/vello_cpu get their own runtime layer-present toggle.
  assert.match(source, /cpuLayerPresent = true/);
  assert.match(source, /await mod\.HayateElementRenderer\.init\(canvas, cpuLayerPresent\)/g);
  // pkg-null must never surface in host-side branching — it has no CanvasBackend.
  assert.doesNotMatch(source, /hayate-adapter-web-null/);
  // no separate layer-present package remains (ADR-0140/#718).
  assert.doesNotMatch(source, /hayate-adapter-web-layer-present/);
  assert.match(source, /AUTO-GENERATED/);
});
