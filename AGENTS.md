# AGENTS.md

Guidance for AI agents working in this repository.

## Cursor Cloud specific instructions

### Monorepo overview

HayateProjects is a **pnpm workspace** with two products:

| Product | Path | Stack |
|---------|------|-------|
| **Hayate** | `Hayate/` | Rust → WASM (wgpu/Vello), static demo gallery |
| **Tsubame** | `Tsubame/` | TypeScript (SolidJS), Vite dev server |

No databases, Docker, or backend services. E2E is browser + dev/static servers.

### First-time / WASM setup (not in update script)

The VM update script only runs `pnpm install`. For full Canvas/GPU paths, also ensure:

1. **Rust stable** — The image may default to Rust 1.83.0; `wasm-pack` 0.15+ needs stable (1.96+):
   ```bash
   rustup default stable
   rustup target add wasm32-unknown-unknown
   cargo install wasm-pack --locked   # once per VM if missing
   ```
2. **Native Hayate tests** — `libfontconfig1-dev` (Debian/Ubuntu) for `cargo test -p hayate-core` on the host target.
3. **Build** — See root `package.json` scripts:
   ```bash
   pnpm run build:tsubame   # required before tests / hello-world
   pnpm run build:wasm      # required for Canvas mode + Hayate demos
   ```

### Running services

| Command | Port | Purpose |
|---------|------|---------|
| `pnpm run dev:hello` | 5173 | Tsubame Hello World (`?mode=dom` or `?mode=canvas`) |
| `pnpm run dev` | 3000 | Builds WASM then serves `Hayate/examples/web-demo` |
| `pnpm --filter hayate run serve` | 3000 | Static gallery only (after `build:wasm`) |

Use explicit mode in the hello-world URL — bare `http://localhost:5173/` can show a blank screen until a renderer is selected.

### Verify

```bash
pnpm run typecheck          # Tsubame packages
pnpm test                   # Vitest (renderer-dom, renderer-canvas)
cd Hayate && cargo test -p hayate-core   # 44+ Rust unit tests (needs fontconfig on host)
```

### Gotchas

- **DOM-only path** skips WASM entirely (`?mode=dom`).
- **Canvas mode** needs `pnpm run build:wasm` and Hayate artifacts under `Hayate/examples/web-demo/pkg/`.
- `preinstall` runs `scripts/bootstrap-wasm-pkgs.mjs` — stub WASM packages for fresh clones before a real WASM build.
- Long-running dev servers should use **tmux** (e.g. session `vite-hello-world`).
