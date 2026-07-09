// check-wasm-not-stub.lib.mjs — pure core for the fail-closed stub guard (#771).
//
// On a fresh clone, preinstall (bootstrap-wasm-pkgs.mjs) writes STUB packages so
// `pnpm install` works before any wasm build. Those stubs must never be published
// (npm unpublish is heavily restricted — a stub release can't be taken back), so
// the release workflow runs this guard AFTER the real wasm build and refuses to
// publish if any public wasm target still carries a stub. Kept free of fs/process
// concerns so the assessment logic is unit-tested against fixtures.

// The stub JS throws with this exact message (bootstrap-wasm-pkgs.lib.mjs JS_STUB).
// Its presence is the unambiguous fingerprint of an un-built wasm target.
export const STUB_MARKER = 'WASM not built';

// One wasm target's verdict. Public targets only — pkg-null (private) is skipped
// by the caller since it is never published.
export function assessTarget({ npmName, wasmExists, jsContent }) {
  if (!wasmExists) {
    return { npmName, ok: false, reason: 'missing hayate_adapter_web_bg.wasm (never built — bootstrap stub)' };
  }
  if (typeof jsContent === 'string' && jsContent.includes(STUB_MARKER)) {
    return { npmName, ok: false, reason: `JS still contains the bootstrap stub marker "${STUB_MARKER}"` };
  }
  return { npmName, ok: true };
}

// The public wasm targets to check: everything the release would publish, i.e.
// not marked private in the manifest (pkg-null is private → excluded).
export function publicWasmTargets(manifest) {
  return manifest.targets.filter((t) => !t.private);
}
