# __PROJECT_NAME__

A [Torimi](https://www.npmjs.com/package/torimi) app (Solid) — a GPU-native UI
that runs on the Torimi host, like an Expo Go project runs on Expo Go.

## Develop

```sh
npm install
npm run dev        # native: build → serve → live-reload; open the Torimi host and connect
# or
npm run dev:web    # web host path
```

`torimi dev` builds the App Bundle, serves it, and live-reloads on every save.
Point the Torimi host at the printed LAN URL (or scan the QR).

## Build

```sh
npm run build      # native App Bundle (Hermes-lowered)
npm run build:web  # web App Bundle
```

## Where to edit

- `src/App.tsx` — your UI, written in Tsubame's element vocabulary (`view`,
  `text`, `text-input`, `button`, …) with Hayate CSS styles.
- `src/main.bundle.tsx` — the single entry (`registerTorimiApp`). You rarely
  touch this.
- `torimi.config.mjs` — the `build`/`bundle` the CLI runs.

## Versions

Every `@tsubame/*` / `@torimi/*` / `@hayate/*` / `torimi` package is on **one
lockstep version**. If you see a Protocol Version mismatch, line them all up to
the same version (and match your Torimi host).
