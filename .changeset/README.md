# Changesets

This directory holds [changesets](https://github.com/changesets/changesets) — one
Markdown file per pending change, describing what changed and how it bumps the
version.

**Lockstep train (ADR-0007 §3).** Every public package is in a single `fixed`
group in `config.json`, so one release bumps them all to the **same** version. A
consumer only ever has to line up "one version for everything" — there is no
per-package compatibility matrix.

## Adding a changeset

```sh
pnpm changeset
```

Pick any public package and a bump type; because the whole closure is `fixed`,
the chosen bump applies to the entire train.

## Releasing

You do **not** publish from your machine (ADR-0007 §4). See
[docs/releasing.md](../docs/releasing.md): merge the changesets "Version Packages"
PR and the GitHub Actions release workflow builds wasm, checks for stubs, and
publishes with npm provenance.
