# @torimi/tsubame-browser-host

The browser App-layer implementation of Tsubame's `Host` port. It owns DOM/Hayate target selection, browser surfaces, and their shared session lifetime while keeping framework entries and `@torimi/tsubame-app` blind to concrete renderers and the Hayate runtime.

`renderer=dom` and browsers without EditContext select the DOM target. Every other renderer query value stays opaque and enters Hayate, where the Rust Render Host owns Scene Renderer selection.

Part of the Torimi/Tsubame lockstep release train. Alpha (0.x): no backward-compatibility guarantees.
