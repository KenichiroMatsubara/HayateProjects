# @tsubame/renderer-dom

The DOM Renderer: a pure-JS `IRenderer` implementation of the Tsubame Renderer Protocol that reflects directly into the browser DOM. It does not use Hayate (no WASM) and is CSR-only — no SSR, no hydration. It is the DOM peer of the Hayate (Canvas) renderer, and the two are distinguished by target.

App authors normally get this transitively through a Tsubame Adapter (`@tsubame/solid` / `@tsubame/react`) and a composition root rather than installing it directly.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
