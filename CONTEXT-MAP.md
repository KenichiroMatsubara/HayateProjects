# Context Map

This monorepo has multiple domain contexts. Read the `CONTEXT.md` relevant to the area you are working in.

| Context | Path | Scope |
| ------- | ---- | ----- |
| **Hayate / Hayabusa (ecosystem)** | [`CONTEXT.md`](./CONTEXT.md) | Cross-cutting product language, Element Layer, Hayabusa SFC framework |
| **Hayate (Rust/WASM core)** | [`Hayate/CONTEXT.md`](./Hayate/CONTEXT.md) | Element Document Runtime, SceneGraph, adapters, Hayate CSS |
| **Tsubame (JS/TS renderer)** | [`Tsubame/CONTEXT.md`](./Tsubame/CONTEXT.md) | Renderer Protocol, DOM/Canvas renderers, tsubame-solid |

## ADR layout

| Scope | Location |
| ----- | -------- |
| Monorepo / system-wide | [`docs/adr/`](./docs/adr/) |
| Hayate-specific | [`Hayate/docs/adr/`](./Hayate/docs/adr/) |
| Tsubame-specific | [`Tsubame/docs/adr/`](./Tsubame/docs/adr/) |
