# @torimi/tsubame-app

Tsubame's composition root. `runTsubameApp(host, mount)` folds `Host` wiring, renderer acquisition, and mount behind one interface; the `Host` produces the renderer and Tsubame stays blind to the concrete renderer and the Hayate runtime. Its web-only `shouldUseDomRenderer` helper handles only the explicit DOM escape and missing-EditContext fallback. Canvas backend vocabulary, query parsing, and selection policy belong to `@torimi/hayate-host`.

It depends only on `@torimi/tsubame-renderer-protocol` — never on `@torimi/tsubame-renderer-dom`, `@torimi/tsubame-renderer-hayate`, or `@torimi/hayate-host` (ADR-0012).

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
