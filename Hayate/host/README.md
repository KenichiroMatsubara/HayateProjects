# @hayate/host

Hayate's JS host glue. On web it does the **host bootstrap** — WebGPU probe, backend select, WASM load, and surface acquisition (`createHayateWebHost(canvas)`); on native it pumps an injected `RawHayate` (`./native`, `createHayateNativeHost(raw)`). Either way it returns a `RawHayate` (plus `requestFrame` / `cancelFrame`) that a composition root hands to Tsubame's host-blind Hayate Renderer.

It sits on the Hayate side of the Hayate–Tsubame boundary. App authors rarely install it directly — it is consumed by a composition root or a Torimi host such as `@torimi/host-web`.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
