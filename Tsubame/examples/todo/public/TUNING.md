# Live taste-constant tuning (`tuning.json`)

Edit a JSON file, press **F5**, see the new value — no WASM rebuild. For
feel/visual calibration on a real device (#353 scroll physics, #411 scrollbar,
etc.) without the recompile loop.

## How

1. Copy the template to the live (gitignored) file:
   ```
   cp public/tuning.example.json public/tuning.json
   ```
2. Run the dev server and open Canvas Mode on the device:
   ```
   pnpm --filter @tsubame/example-todo dev -- --host 0.0.0.0
   # phone (same LAN, plain http → WebGPU off, so force the CPU backend):
   #   http://<PC-IP>:<port>/?renderer=tiny-skia
   ```
3. Edit a value in `public/tuning.json`, **F5** the page. Delete the file and
   F5 to fall back to the compiled defaults.

The physics is backend-independent, so values tuned under `tiny-skia` apply
unchanged under `vello`.

## Source of truth

The Rust `const`s are authoritative and ship to production; `tuning.json` is a
**dev-only override** (it is gitignored and never bundled meaningfully). Once a
value feels right, **bake it back into the Rust const** and commit — that is the
acceptance criterion for #353 ("最終値を名前付き定数に反映").

- Scroll physics consts: `Hayate/crates/adapters/web/src/scroll_drag.rs`
- Chrome consts: `Hayate/crates/core/src/element/scene_build.rs`

## Keys

All keys are optional — include only the ones you are tuning. Unknown keys are
rejected (a typo errors out rather than silently doing nothing), and any parse
error makes the whole file fall back to defaults. Colors are `[r, g, b, a]` in
`0..1`.

`scroll.*` — full set is live (slop, deceleration_rate, max_release_velocity,
min_velocity, sample_window_ms, rubber_band_c, spring_stiffness, spring_damping,
spring_rest_offset, spring_rest_velocity).

`chrome.*` — live: scrollbar thickness / track_margin / min_thumb_length /
thumb_color / thumb_opacity, indicator thickness / color / opacity,
selection_highlight_color, composition_underline_thin / thick, placeholder_alpha,
toolbar_corner_radius.

### Not overridable here (v1) — require a recompile

These are owned by the switchable theme or the selection layout pass, which the
override deliberately does not reach:

- Selection **handle** and **toolbar** *colors* and the toolbar **height** /
  **label font size** — owned by `SelectionChromeStyle` (Material/Cupertino) in
  `selection_chrome.rs`.
- Handle **hit radius**, toolbar **gap** / label advance — selection layout
  geometry.
- Touch indicator **fade timing** (`*_HOLD_MS` / `*_FADE_MS`) and scrollbar
  **page step** — read by paths without tree access.
