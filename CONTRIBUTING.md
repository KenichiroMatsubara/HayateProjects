# Contributing to Hayate Projects

Thanks for looking in. This is an active, pre-release R&D monorepo — the most
valuable contributions right now are **feedback, reproductions, and small focused
fixes**, not large features (the architecture is still moving).

## Ways to help (in order of usefulness right now)

1. **Try the demos and tell us what broke or confused you.** Open an issue with
   your OS, browser, and what you expected vs. saw. A confused first-run report is
   genuinely useful signal.
2. **Pick up a [`good first issue`](https://github.com/KenichiroMatsubara/HayateProjects/issues).**
3. **Improve docs.** If a README or spec left you stuck, a PR that unsticks the
   next person is welcome.

## Getting set up

Prerequisites: **Node 20+**, **pnpm**, and a **Rust toolchain** (for the wasm core).

```sh
git clone https://github.com/KenichiroMatsubara/HayateProjects.git
cd HayateProjects
pnpm install          # runs the wasm bootstrap automatically

pnpm dev              # Tsubame Todo demo (DOM/Canvas switch)
pnpm build            # build all public packages
pnpm typecheck
pnpm test
```

If you're only touching one area, read that area's `CONTEXT.md` first
(see [CONTEXT-MAP.md](./CONTEXT-MAP.md)) — this project keeps a canonical
vocabulary and PRs are expected to use it.

## Before you open a PR

- **Match the surrounding code** — naming, comment density, and the domain
  vocabulary in `CONTEXT.md`.
- **Keep changes focused.** One concern per PR.
- **Add a changeset** if your change touches a published package:
  `pnpm changeset` (see [docs/releasing.md](./docs/releasing.md)). No changeset =
  the release train doesn't move, which is fine for docs-only PRs.
- **Run `pnpm typecheck` and `pnpm test`** locally.
- **Significant design changes** should reference or add an
  [ADR](./docs/adr/). If in doubt, open an issue to discuss before building.

## Reporting bugs / asking questions

Open a [GitHub issue](https://github.com/KenichiroMatsubara/HayateProjects/issues).
Include a minimal reproduction where you can. For rendering bugs, note which
renderer (`dom` / `tiny-skia` / `vello`) and whether WebGPU is available.

## License

By contributing, you agree your contributions are licensed under Apache-2.0, the
same as the project.
