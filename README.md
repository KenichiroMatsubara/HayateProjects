# Hayate Projects

**React Native, rebuilt on a Rust + GPU core.**

Hayate Projects is a monorepo building a GPU-native UI stack from the ground up: a
Rust rendering foundation, a JS/TS renderer layer that lets existing frameworks
target it, and an "Expo Go"-style dev client for running apps on a device with
live reload.

> 🇯🇵 日本語の要約は [下の「日本語」節](#日本語) を参照。

> ⚠️ **Status: pre-release / alpha (0.x).** The core renders, the JS layer has a
> working MVP, and the dev-client loop runs — but nothing is published to a package
> registry yet and APIs break freely between 0.x iterations. See
> [Status](#status) for the honest, per-part breakdown.

---

## Why this exists

Today's cross-platform UI splits into two camps, each with a cost:

- **React Native / Expo** reuse the JS ecosystem, but each platform's UI is
  rendered by the OS's own widgets — so behaviour diverges per platform and you
  inherit the DOM/native quirks you were trying to escape.
- **Flutter** gets pixel-identical GPU rendering, but forces one language (Dart)
  and one framework on you.

Hayate takes a deliberately contrarian third path:

1. **One GPU renderer, identical everywhere.** A single Rust core
   (`wgpu` + `Vello` + `Taffy` + `parley`) draws the UI, so a button looks and
   behaves the same on every target. No OS widgets, no per-platform divergence.
2. **Bring your own framework.** The JS layer (**Tsubame**) doesn't ship a new
   framework — it lets *SolidJS / React / Vue keep their own runtimes* and just
   re-targets where they render. You keep the framework you know.
3. **…or bring your own language.** The Rust-native framework (**Hayabusa**) owns
   its reactive runtime in Rust and compiles single-file components (`.hybs`) —
   the exact opposite bet from Tsubame, for when you want to leave JS entirely.

The wager: **the rendering engine is the hard, reusable part; the framework and
the language should be your choice, not the engine's.**

---

## The four projects

```
        ┌─────────────────────────────────────────────────────────┐
        │  Your app: SolidJS · React · Vue   |   .hybs (Rust-native)│
        ├───────────────────────────┬─────────────────────────────┤
        │   Tsubame (≈ React Native)│   Hayabusa (Signal SFC, Rust)│
        │   Renderer Protocol +      │   owns its reactive runtime  │
        │   DOM / Canvas renderers    │                             │
        ├───────────────────────────┴─────────────────────────────┤
        │              H A Y A T E  —  GPU-native UI core           │
        │        Element Layer  ·  layout  ·  style  ·  paint        │
        ├───────────────────────────────────────────────────────────┤
        │            wgpu  →  WebGPU / Vulkan / Metal / DX12         │
        └───────────────────────────────────────────────────────────┘

        Torimi (≈ Expo Go):  dev server + App Bundle + native host + live reload
```

| Project | One line | Analogy |
| --- | --- | --- |
| **Hayate** (疾風) | Imperative, retained, GPU-native UI foundation. Rust core that takes an element tree + inline styles and paints it via `wgpu`/`Vello`. Not a framework. | The rendering engine |
| **Hayabusa** (隼) | Signal-based single-file-component framework that owns its reactive runtime *in Rust*. Components are `.hybs` files. | A framework, but Rust-native |
| **Tsubame** (燕) | JS/TS renderer-target layer. A `Renderer Protocol` with DOM and Canvas backends; adapters let SolidJS/React/Vue render into Hayate's GPU canvas without changing framework. | ≈ React Native's targeting, framework-agnostic |
| **Torimi** (鳥見) | Framework-agnostic dev client: bundles your app, serves it with live reload + QR code, and a native host renders it on-device. | ≈ Expo Go + the `expo` CLI |

Each has its own deep-dive README: [Hayate](./Hayate/README.md) ·
[Hayabusa](./Hayabusa/README.md) · [Tsubame](./Tsubame/README.md) ·
[Torimi](./Torimi/cli/README.md).

---

## Status

Honest, per-part. This is an active R&D project by a solo author, not a shipped product.

| Part | State |
| --- | --- |
| **Hayate core** | 🚧 Step 1 — Canvas Mode (WebGPU + Vello) rendering to a browser canvas; Android & desktop hosts exist. HTML Mode fallback works. |
| **Tsubame** | ✅ MVP done — Renderer Protocol, DOM Renderer, Canvas Renderer, `tsubame-solid`, and the Todo demo. `tsubame-react` / `tsubame-vue` partial. |
| **Hayabusa** | 🚧 Reactive core, expression DSL, `.hybs` compiler, and live Hayate-core integration proven via slices; not yet an app-ready framework. |
| **Torimi** | 🚧 CLI, dev server, App Bundle, `create-torimi` scaffolder, and an Android host exist; end-to-end dev loop runs inside the monorepo. |
| **npm packages** | ❌ Not published yet. The full release pipeline is designed ([ADR-0007](./docs/adr/0007-npm-alpha-distribution.md)) but the `0.1.0` train hasn't left the station. |

---

## Try it (inside the monorepo)

Until the npm alpha ships, the way to see it running is from a clone.

```sh
# Prereqs: Node 20+, pnpm, and a Rust toolchain (for the wasm core)
pnpm install

# DOM / Canvas one-button-switch Todo demo (Tsubame Task Studio)
pnpm dev

# The draw-gallery demo (same painter on GPU and DOM paths)
pnpm --filter @tsubame/example-draw-gallery dev
```

Renderer is switchable at runtime via the top-right toggle or a URL query:
`?renderer=dom` (no WebGPU needed) · `?renderer=tiny-skia` (CPU backend) ·
`?renderer=vello` (WebGPU).

For the full `torimi dev` device loop, see the [Torimi CLI README](./Torimi/cli/README.md).

---

## Architecture & decisions

This project is documented unusually deeply for its size. If you want to
understand *why* it's shaped this way:

- **[CONTEXT-MAP.md](./CONTEXT-MAP.md)** — where each domain's vocabulary lives.
- **[CONTEXT.md](./CONTEXT.md)** — the canonical product vocabulary (what each term *is*).
- **[docs/spec/](./docs/spec/)** — the system, core, layout, rendering, protocol specs.
- **[docs/adr/](./docs/adr/)** — architecture decision records (the reasoning trail).

---

## Contributing

Contributions and feedback are welcome — see **[CONTRIBUTING.md](./CONTRIBUTING.md)**.
The fastest way in: run the demos above, open an issue with what confused you, or
pick up a [`good first issue`](https://github.com/KenichiroMatsubara/HayateProjects/issues).

## License

Apache-2.0 (see `Hayate/LICENSE`, `Tsubame/LICENSE`). Vendored dependencies
(`Vello`, `Taffy`, `parley`, etc.) retain their own MIT/Apache-2.0 licenses.

---

## 日本語

**Hayate Projects は「React Native を Rust + GPU コアで作り直す」モノレポです。**

OS のネイティブ widget に描画を委ねる React Native や、言語・フレームワークを固定する
Flutter に対し、Hayate は第三の逆張りを取ります：**描画は単一の Rust + GPU コア
（`wgpu`/`Vello`）に一本化して全ターゲットで同一の見た目・挙動を保証しつつ、フレーム
ワークと言語は利用者が選べる**、という賭けです。

- **Hayate（疾風）** — 命令型・保持型・GPU ネイティブな UI 基盤（Rust コア）。フレーム
  ワークではなく描画エンジン。
- **Hayabusa（隼）** — リアクティブランタイムを Rust で単独所有する Signal 型 SFC
  フレームワーク（`.hybs`）。Tsubame と対をなす意図的な逆張り。
- **Tsubame（燕）** — JS/TS レンダラーターゲット層。SolidJS / React / Vue が自前の
  ランタイムのまま Hayate の GPU canvas に描画先を向け替える（≈ React Native）。
- **Torimi（鳥見）** — フレームワーク非依存の dev-client。バンドル・ライブリロード・
  QR・ネイティブホストで実機表示（≈ Expo Go ＋ `expo` CLI）。

**現状はアルファ（0.x）**。コアは描画でき、Tsubame は MVP 完了、Torimi の dev ループは
モノレポ内で動きますが、**npm 未公開**で API も 0.x 間で破壊的に変わります。詳細は上の
[Status](#status) を参照。動かし方は [Try it](#try-it-inside-the-monorepo) の通り。

設計の背景は [CONTEXT.md](./CONTEXT.md) と [docs/adr/](./docs/adr/) に厚く残しています。
