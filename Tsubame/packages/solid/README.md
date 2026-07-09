# @tsubame/solid

A Tsubame Adapter. It keeps SolidJS's own runtime (fine-grained signals) and retargets `solid-js/universal` onto the Tsubame Renderer Protocol (`IRenderer`), so the same UI renders to either the DOM or the Hayate (Canvas) renderer. It holds no drawing source of truth — only a structure-only shadow index required by `solid-js/universal`'s synchronous tree walk.

It also ships the `@tsubame/solid/vite` preset for build integration.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
