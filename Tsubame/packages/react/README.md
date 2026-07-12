# @torimi/tsubame-react

A Tsubame Adapter. It keeps React's own Fiber runtime (hooks, Suspense, Context) and retargets `react-reconciler` onto the Tsubame Renderer Protocol (`IRenderer`), so the same UI renders to either the DOM or the Hayate (Canvas) renderer. It holds no drawing source of truth — it delivers `ElementId` handles and mutations to an `IRenderer`.

It also ships the `@torimi/tsubame-react/vite` preset for build integration.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
