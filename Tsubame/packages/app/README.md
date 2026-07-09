# @tsubame/app

Tsubame's composition root. `runTsubameApp(host, mount)` folds target selection, `Host` wiring, renderer acquisition, and mount behind one interface; the `Host` produces the renderer and Tsubame stays blind to the concrete renderer and the Hayate runtime. It also ships `detectMode`, a dependency-free web-only DOM/Canvas decision helper.

It depends only on `@tsubame/renderer-protocol` — never on `@tsubame/renderer-dom`, `@tsubame/renderer-hayate`, or `@hayate/host` (ADR-0012).

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
