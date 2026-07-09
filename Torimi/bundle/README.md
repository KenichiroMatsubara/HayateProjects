# @torimi/bundle

Bundle Registration: the App Bundle's composition root. An app calls `registerTorimiApp(mount)` as its single all-targets entry, and this package hides the wire-contract wiring — baking in the protocol version, registering the mount seam (`__torimiMount` / `__tsubame`), and the native prelude. Target differences are branched at runtime on the presence of `__hayateHost`; framework knowledge is received only as the `TsubameMount` argument (FW-blind). Internally it calls Tsubame's `runTsubameApp`.

It also ships the `@torimi/bundle/vite` App Bundle preset.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
