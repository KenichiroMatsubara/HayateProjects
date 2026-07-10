# @torimi/dev-server-contract

The wire contract between the Torimi dev server and the web host: the App Bundle route, the reload route, the reload message, and the Device Log route prefix with its `LogBatch`/`LogEntry` types (ADR-0005). It is the single entry point that `@torimi/dev-server` and `@torimi/host-web` both reference as equals (ADR-0001).

This is a contract package consumed transitively by the dev server and host, not installed directly by app authors.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
