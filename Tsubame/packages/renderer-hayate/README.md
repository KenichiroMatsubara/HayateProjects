# @torimi/tsubame-renderer-hayate

The Hayate (Canvas) Renderer: an `IRenderer` implementation of the Tsubame Renderer Protocol. It accumulates a frame's worth of mutations on the JS side and hands them to Hayate's WASM `apply_mutations` once per frame. It is host-blind — it receives only the frame-clock tick, while surface, resize, pointer, and IME are owned by the host-built adapter.

App authors normally get this transitively through a Tsubame Adapter (`@torimi/tsubame-solid` / `@torimi/tsubame-react`) and a composition root rather than installing it directly.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
