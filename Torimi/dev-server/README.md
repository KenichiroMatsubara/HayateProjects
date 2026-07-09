# @torimi/dev-server

Torimi's minimal dev server: it serves a single App Bundle over HTTP, watches it, and relays reload signals to connected hosts over WebSocket. It is framework- and build-tool independent — the bundle is an opaque single JS file it never inspects.

It sits below the `torimi` CLI, which orchestrates the build and drives this server; the wire routes and messages it speaks live in `@torimi/dev-server-contract`.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
