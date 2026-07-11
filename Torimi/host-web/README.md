# @torimi/host-web

The Torimi Web host: it fetches and evals the App Bundle from the dev-server URL, then establishes the host bootstrap via `@torimi/hayate-host` (`createHayateWebHost`) and hands it to the bundle's mount. It speaks the same dev-server and mount contracts as the native host but consumes the web App Bundle. It carries no framework and no `@torimi/tsubame-renderer-hayate` of its own.

It is one of the two Torimi host kinds and is used mainly for E2E tests and quick on-machine checks.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
