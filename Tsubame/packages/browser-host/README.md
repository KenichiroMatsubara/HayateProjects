# @torimi/tsubame-browser-host

The browser App-layer implementation of Tsubame's `Host` port. It owns DOM/Hayate target selection, browser surfaces, and their shared session lifetime while keeping framework entries and `@torimi/tsubame-app` blind to concrete renderers and the Hayate runtime.

`renderer=dom` and browsers without EditContext select the DOM target. Every other renderer query value stays opaque and enters Hayate, where the Rust Render Host owns Scene Renderer selection.

Hayate startup is one resource transaction: the selected surface is revealed before optional tuning is loaded, then the Worker-backed Web Host and renderer are started. Disposal always attempts `renderer.stop`, `webHost.detach`, and visibility restoration in reverse order. Development tuning is opt-in:

```ts
createBrowserHost({
  dom,
  canvas,
  tuning: { kind: 'optional-url', url: '/tuning.jsonc' },
});
```

Omitting `tuning` performs no fetch. Use `{ kind: 'inline', json }` for injected tuning or `{ kind: 'none' }` explicitly. `host.inspection()` exposes only the selected target and pipeline observation; callers may publish that object under their own development-only seam without exposing RawHayate or Worker resources.

Part of the Torimi/Tsubame lockstep release train. Alpha (0.x): no backward-compatibility guarantees.
