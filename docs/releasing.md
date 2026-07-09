# Releasing (npm lockstep train)

How the public npm packages are released. One release bumps **every** public
package to the **same** version (lockstep train, ADR-0007 §3). Publishing happens
**only** in GitHub Actions — never from a developer's machine (ADR-0007 §4).

## The one-paragraph version

1. Land your change with a **changeset** describing it.
2. GitHub Actions opens a **"Version Packages"** PR that bumps the whole train.
3. **Merge that PR.** The release workflow builds Rust → wasm → JS, verifies no
   wasm stub is about to ship, and publishes the whole closure to npm with
   provenance. Done.

## 1. Add a changeset with your change

```sh
pnpm changeset
```

Pick a bump type (for 0.x alpha, `patch` or `minor`). Because every public
package is in one `fixed` group (`.changeset/config.json`), the bump applies to
the **entire train** — you don't select packages individually. Commit the
generated file under `.changeset/` with your PR.

No changeset = no release. A PR with no changeset just doesn't move the train.

## 2. Merge the "Version Packages" PR

When your PR lands on `main`, the release workflow (`.github/workflows/release.yml`)
runs [`changesets/action`](https://github.com/changesets/changesets), which opens
or updates a **"Version Packages"** PR. That PR:

- bumps every public package's `version` to the next train number,
- rolls the changeset files into `CHANGELOG.md` entries.

Review it like any PR, then **merge it**.

## 3. Automatic publish

Merging the Version PR leaves no pending changesets, so on that push the workflow
**publishes** instead of re-opening the PR. It:

1. builds the real wasm (`pnpm --filter hayate run build:all`) — the fresh-clone
   bootstrap stubs are **not** publishable,
2. runs the **fail-closed stub check** (`pnpm run check:wasm-not-stub`): if any
   public wasm target still carries a stub, the release aborts before npm,
3. builds the JS closure,
4. runs `pnpm release` → `changeset publish` with **npm provenance**
   (`NPM_CONFIG_PROVENANCE=true`) and dist-tag `latest`.

### Required secret

- `NPM_TOKEN` — an npm automation token with publish rights to the `@hayate`,
  `@tsubame`, `@torimi` scopes and the unscoped `torimi` / `create-torimi`
  packages. Set it in the repo's Actions secrets. Without it the **dry-run** still
  runs green (it stops right before publishing); only the real publish needs it.

## Guards (why you can't publish by hand)

- **CI-only publish.** Every public package has a `prepublishOnly` that exits
  non-zero unless `GITHUB_ACTIONS` is set. `pnpm publish` on a laptop fails fast.
- **No stub ships.** `scripts/check-wasm-not-stub.mjs` fails closed if a wasm
  target is still a bootstrap stub (npm's unpublish restrictions make a stub
  release unrecoverable).
- **Dry-run on every PR.** The `dry-run` job runs the whole chain up to
  `pnpm -r publish --dry-run`, so packing/metadata breakage is caught before merge
  without needing `NPM_TOKEN`.

## Version ↔ Torimi host

The lockstep train number lines up 1:1 with the Torimi host (Play) — "host vX ⇔
0.x train" — the same way an Expo SDK version lines up with its client. A
`Protocol Version` mismatch at runtime means the app and host are on different
trains: line the versions up (see the closure's READMEs).
