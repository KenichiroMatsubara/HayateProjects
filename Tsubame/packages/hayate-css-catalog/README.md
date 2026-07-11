# @torimi/tsubame-hayate-css-catalog

The Hayate CSS Catalog: the protocol-spec-driven source of truth for style props, shared so the Canvas encode path, the DOM CSS path, and the Rust mapper all agree on one vocabulary and its semantics. It is derived from the protocol spec rather than hand-maintained per renderer.

This is a shared catalog consumed transitively by the renderers and codegen, not installed directly by app authors.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
