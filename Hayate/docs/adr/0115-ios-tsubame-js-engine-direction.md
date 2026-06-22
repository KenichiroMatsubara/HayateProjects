# iOS Tsubame JS Engine Direction: Hermes for Parity (Policy Only)

**Status: Draft**

**Date: 2026-06-22**

## Context

ADR-0112 (Draft) embeds Hermes in the Android adapter and drives Tsubame JS over JSI, with
the apply path (`apply_mutations_batch`) shared with Web once the proto codegen is
neutralised (ADR-0055). ADR-0113/0114 brought up the iOS adapter's native render/touch/IME
groundwork but explicitly left the Tsubame JS path to policy only — no code this round.

When iOS runs Tsubame JS (`tsubame-solid` + `@tsubame/renderer-canvas` + the Todo example)
calling **native** Hayate, it needs a JS engine embedded in the staticlib plus a host bridge
satisfying `RawHayate` (`Tsubame/packages/renderer-canvas/src/hayate.ts`). The Tsubame↔Hayate
boundary is deliberately coarse (ADR-0052): per frame the JS crosses only a few times with
batched arrays, so the host bridge is a fixed ~15-method surface, not a per-frame cost. As on
Android, the engine choice therefore turns on tooling/longevity, not per-call marshalling.

Engine options for iOS:

1. **Hermes (JSI)** — same engine and host model as Android (ADR-0112). Hermes builds for
   iOS/arm64 (React Native ships it on iOS), bytecode AOT via `hermesc`, a real debugger, and
   one engine + one JSI host across both native platforms. Cost: a second build graph
   (link Hermes, a C++ JSI TU, a `cxx` bridge) — but written once.
2. **System JavaScriptCore (`JSContext`)** — always present on iOS, zero added binary, no
   vendored engine. But its host API differs from JSI, so the bridge would *fork* from
   Android's JSI host; no bytecode AOT; and Apple restricts JIT to the system framework
   (fine for `JSContext`, but it's a different integration than Android's).

## Decision (policy only — no code this round)

- **Recommended direction: embed Hermes (JSI) on iOS too**, for parity with Android
  (ADR-0112). One JS engine and one JSI host across Android + iOS, the shared
  platform-neutral `apply_mutations` dispatch (ADR-0055/0112), and the shared
  `init-*`/`main.*.tsx` discipline. The coarse boundary neutralises Hermes' only real
  downside (the C++/JSI host is written once), leaving its longevity/tooling advantages
  decisive — the same reasoning as ADR-0112.
- **Record JavaScriptCore as the fallback**: revisit only if binary-size pressure or
  Hermes-on-iOS build friction outweighs the cost of forking the bridge from Android's JSI
  host. It is not chosen now because it would split the native JS integration in two.
- **Non-destructive and staged when implemented**: gate behind a Cargo feature
  (`tsubame-js`, mirroring Android), keep `build_demo_tree` as the default fallback, and
  follow Android's staged scope (eval round-trip → one static frame → full Solid+Todo →
  touch via `poll_events` → IME via the existing UITextInput bridge). The proto codegen
  neutralisation (ADR-0112) is a shared prerequisite, done once for both platforms.
- **Status stays Draft** until the Android Hermes path (ADR-0112, also Draft) is validated
  on a device, so iOS can reuse its proven JSI host rather than co-developing two unproven
  ones.

## Consequences

### Positive

- One JS engine / JSI host / apply path across both native platforms; iOS reuses Android's
  bridge instead of inventing a second integration.
- Decision is captured now so the iOS groundwork (ADR-0113/0114) has a known JS direction,
  without committing code while Android's path is still Draft.

### Negative

- Inherits Hermes' second-build-graph cost on iOS (Hermes framework, C++/JSI TU, `cxx`,
  `hermesc` bundling) — heavier than the free system JavaScriptCore.
- Couples iOS's JS timeline to ADR-0112 maturing; if Hermes-on-iOS proves problematic, the
  JavaScriptCore fallback means re-forking the bridge.
- No iOS JS code exists yet; this ADR only fixes direction.
