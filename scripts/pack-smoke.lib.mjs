// pack-smoke.lib.mjs — pure, testable core for the npm pack smoke test (#768).
//
// The smoke test packs every public package, installs the tarballs in a throwaway
// project OUTSIDE the monorepo, and imports the three entry points an external app
// touches first. This module holds the parts with no process/fs concerns so they
// can be unit-tested: the definition of "the public closure" and the shape of the
// throwaway consumer's package.json.

// The npm publish closure (ADR-0007 §1). This is the single source of truth the
// workspace is checked against: a package that forgets to drop `private`, or one
// that should stay private but doesn't, diverges from this list and fails the
// guardrail test. `torimi` (#770) and `create-torimi` (#772) join this list when
// those slices land.
export const EXPECTED_PUBLIC_PACKAGES = [
  '@hayate/adapter-web',
  '@hayate/adapter-web-cpu',
  '@hayate/adapter-web-vello-cpu',
  '@hayate/host',
  '@hayate/protocol-spec',
  '@torimi/bundle',
  '@torimi/dev-server',
  '@torimi/dev-server-contract',
  '@torimi/host-web',
  '@torimi/protocol-handshake',
  '@tsubame/app',
  '@tsubame/hayate-css-catalog',
  '@tsubame/protocol-generated',
  '@tsubame/react',
  '@tsubame/renderer-dom',
  '@tsubame/renderer-hayate',
  '@tsubame/renderer-protocol',
  '@tsubame/solid',
];

// The three imports an external app reaches for first: the FW adapter, the host
// glue (the un-hideable direct dep, ADR-0004), and the dev server. If the packed
// tarballs install and these resolve outside the monorepo, the closure is whole.
export const SMOKE_IMPORTS = ['@tsubame/solid', '@hayate/host', '@torimi/dev-server'];

// `pnpm ls -r --depth -1 --json` rows → the names pnpm would publish. A package is
// public iff it is not private; unnamed rows (defensive) are skipped.
export function publicPackages(rows) {
  return rows
    .filter((row) => typeof row.name === 'string' && row.private === false)
    .map((row) => row.name)
    .sort();
}

// The throwaway consumer's package.json. The three SMOKE_IMPORTS are direct file:
// deps; every public tarball is pinned through pnpm `overrides` so the inter-package
// deps (e.g. @hayate/host → @hayate/adapter-web) resolve to the LOCAL tarballs and
// never touch the network — the whole closure installs offline, exactly as an
// external consumer would get it from a registry after a real release.
export function buildSmokeProjectManifest(tarballs) {
  const overrides = {};
  for (const [name, tgz] of Object.entries(tarballs)) overrides[name] = `file:${tgz}`;

  const dependencies = {};
  for (const name of SMOKE_IMPORTS) {
    if (!tarballs[name]) throw new Error(`pack-smoke: no packed tarball for smoke import "${name}"`);
    dependencies[name] = `file:${tarballs[name]}`;
  }

  return {
    name: 'pack-smoke-consumer',
    version: '0.0.0',
    private: true,
    type: 'module',
    dependencies,
    pnpm: { overrides },
  };
}
