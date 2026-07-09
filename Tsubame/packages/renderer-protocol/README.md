# @tsubame/renderer-protocol

The Tsubame Renderer Protocol: the `IRenderer` interface and its types — the boundary between a Tsubame Adapter and a renderer (app↔renderer). It abstracts element creation, tree operations, style setting, and event subscription, and is the shared contract implemented by both the DOM renderer and the Hayate (Canvas) renderer.

This is a types/interface package. It is the only dependency of the composition root (`@tsubame/app`) and is pulled in transitively by the adapters and renderers rather than installed directly.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
