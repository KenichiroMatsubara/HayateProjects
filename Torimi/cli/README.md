# torimi

The orchestrator CLI for **Torimi** — build, serve, and live-reload GPU-native
apps on a Torimi host. If Tsubame is "≈ React Native", Torimi is "≈ Expo Go", and
`torimi` is the `expo` command: it runs your app's build, applies the
target-specific lowering, and serves the bundle with live reload and a QR code.

> **Alpha (0.x).** No backward-compatibility guarantees. Everything below can
> change between 0.x releases.

## What Torimi is

You write a UI once (Solid or React, via [Tsubame](https://www.npmjs.com/package/@tsubame/solid)).
`torimi` bundles it into a single **App Bundle** and serves it; a **Torimi host**
app on your device fetches that bundle and renders it on Hayate's GPU canvas —
exactly like Expo Go loads your project. The CLI itself is a bundler-agnostic
orchestrator: it runs the build command *you* declare and never grows framework
or build-tool knowledge.

## Prerequisites

- **A Torimi host** to render into (the native shell, like the Expo Go app). The
  host has a baked-in Protocol Version; keep your packages on the matching train
  (see [Versions](#versions)).
- **Node 20+** and a package manager (npm / pnpm).

## Quickstart

```sh
# 1. Scaffold a new app (bundled template — no network fetch, pinned to this train)
npm create torimi my-app
cd my-app
npm install

# 2. Start the dev loop: build → serve → live-reload on every save
npm run dev            # → torimi dev  (native target)

# 3. Open the Torimi host on your device and connect to the printed LAN URL
#    (or scan the QR). Edit src/App.tsx and save — the host reloads.
```

`npm run dev:web` uses the web host path instead. To produce a shippable bundle:

```sh
npm run build          # → torimi build         (native App Bundle, Hermes-lowered)
npm run build:web      # → torimi build web      (web App Bundle)
```

This is the same flow the published-package smoke test exercises end to end:
`create-torimi` → install → `torimi build`.

## Command reference

| Command | What it does |
| --- | --- |
| `torimi dev [target]` | Build once, watch sources, serve the bundle, and live-reload connected hosts. Default target `native`; for native it lowers the built bundle for Hermes and serves the **lowered** copy (never an un-lowered bundle). |
| `torimi build [target]` | One-shot build. `native` additionally lowers to `<bundle>.hermes.js`. For CI and Demo Endpoint intake. |
| `torimi lower <file>` | Hermes-lower a built bundle in place — the escape hatch when you drive the build yourself. |

Targets: `native` (default) or `web`. The dev server port defaults to **5179**
(native) / **5181** (web); override with `TORIMI_DEV_PORT`.

## `torimi.config.*`

A flat config — no per-target branching (the native/web difference is the CLI's
knowledge, not yours):

```js
// torimi.config.mjs
export default {
  build: 'vite build --config vite.config.torimi.ts', // the opaque one-shot build command
  bundle: 'dist-torimi/bundle.js',                     // where that build writes the App Bundle
  watch: 'src',                                        // sources torimi dev watches (default: 'src')
};
```

| Key | Meaning |
| --- | --- |
| `build` | The build command the CLI runs. Opaque — the CLI never inspects it. |
| `bundle` | The path `build` writes the single App Bundle to. |
| `watch` | Directory `torimi dev` watches for changes. Optional; defaults to `src`. |

## Versions

Every published package — `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`,
`create-torimi` — moves as **one lockstep version train**. Keep them all on the
**same version**, and match your Torimi host's train.

If you hit a **Protocol Version mismatch** at runtime, your app bundle and the
host were built from different trains. The fix is always the same: line every
package up to one version, and use a host from the matching train.

## License

Apache-2.0
