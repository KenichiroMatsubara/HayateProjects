# @torimi/hayate-host

Hayate's JS host glue. On web, `createHayateWebHost(canvas)` transfers the surface to one
OffscreenCanvas Worker. That Worker owns the WASM Core, Render Host, renderer selection, shared
latest-wins frame pipeline, and presentation for its lifetime; the main thread only transports DOM
input, IME presentation, and frame-clock messages. Missing Worker/OffscreenCanvas support and
renderer boot failures are typed errors—there is no main-thread Canvas fallback. On native it pumps
an injected `RawHayate` (`./native`, `createHayateNativeHost(raw)`).

Either way it returns a `RawHayate` (plus `requestFrame` / `cancelFrame`) that a composition root
hands to Tsubame's host-blind Hayate Renderer.

It sits on the Hayate side of the Hayate–Tsubame boundary. App authors rarely install it directly — it is consumed by a composition root or a Torimi host such as `@torimi/host-web`.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
