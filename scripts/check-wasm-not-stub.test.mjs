// check-wasm-not-stub.test.mjs — unit tests for the fail-closed stub guard (#771).
// Run with: node --test scripts/
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { JS_STUB } from './bootstrap-wasm-pkgs.lib.mjs';
import { STUB_MARKER, assessTarget, publicWasmTargets } from './check-wasm-not-stub.lib.mjs';
import { loadManifest } from '../Hayate/scripts/wasm-manifest.mjs';

test('assessTarget fails a target with no built .wasm (the fresh-clone stub state)', () => {
  const v = assessTarget({ npmName: '@hayate/adapter-web', wasmExists: false, jsContent: JS_STUB });
  assert.equal(v.ok, false);
  assert.match(v.reason, /missing/);
});

test('assessTarget fails a target whose JS still carries the bootstrap stub marker', () => {
  // Even if a .wasm somehow exists, stub JS must fail closed.
  const v = assessTarget({ npmName: '@hayate/adapter-web', wasmExists: true, jsContent: JS_STUB });
  assert.equal(v.ok, false);
  assert.match(v.reason, new RegExp(STUB_MARKER));
});

test('assessTarget passes a real built target (wasm present, no stub marker)', () => {
  const realJs = 'export default async function init(){/* wasm-bindgen glue */}\nexport class HayateElementRenderer{}';
  const v = assessTarget({ npmName: '@hayate/adapter-web', wasmExists: true, jsContent: realJs });
  assert.equal(v.ok, true);
});

test('the bootstrap JS_STUB actually contains the marker the guard keys on', () => {
  // If the stub text ever drifts, this fails so the guard is updated in lockstep.
  assert.ok(JS_STUB.includes(STUB_MARKER));
});

test('publicWasmTargets excludes the private pkg-null but includes the scoped public targets', () => {
  const names = publicWasmTargets(loadManifest()).map((t) => t.npmName);
  assert.ok(names.includes('@hayate/adapter-web'));
  assert.ok(names.includes('@hayate/adapter-web-cpu'));
  assert.ok(names.includes('@hayate/adapter-web-vello-cpu'));
  assert.ok(!names.includes('hayate-adapter-web-null'));
});
