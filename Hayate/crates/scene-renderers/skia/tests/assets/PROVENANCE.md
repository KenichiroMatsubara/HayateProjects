# Test font provenance

## `twemoji_smiley_sbix.ttf`

- **Source**: [`googlefonts/color-fonts`](https://github.com/googlefonts/color-fonts),
  file `fonts/twemoji_smiley-sbix.ttf`.
- **License**: Apache License 2.0 (see the upstream repository's `LICENSE`).
- **Purpose**: a tiny (~22 KB) `sbix` (embedded PNG bitmap strikes) colour emoji
  font used to prove that the skia-safe `hayate-scene-renderer-skia` painter draws
  colour glyphs through `SkTextBlob` (ADR-0146 §4, issue #800).
- **Why `sbix` and not `colr_test_glyphs.ttf`** (the COLRv1 gradient font used by
  the shared `hayate-scene-test-support::cases::color_glyph_tree` fixture / the
  Vello backend's colour-glyph test): empirically, the crates.io `skia-safe =
  0.99.0` prebuilt binary's **CPU raster** canvas does not paint COLRv1 gradient
  paint-graph glyphs — it silently falls back to the plain `glyf` outline filled
  with the paint colour (verified 2026-07-12: `COLR`/`CPAL` tables are present and
  parsed, `SkTypeface::getTableSize` reports them, but `SkCanvas::drawTextBlob` on
  a `surfaces::raster()` canvas paints a single flat colour). Bitmap colour glyphs
  (`sbix`, and separately verified with the system `NotoColorEmoji.ttf`'s `CBDT`/
  `CBLC` tables) paint correctly on the same CPU raster canvas with the exact same
  painter code path. This is consistent with COLRv1's gradient/paint-graph
  evaluation needing a shader-capable backend that this Skia CPU build does not
  provide — worth re-testing once the Android Ganesh/EGL (GPU) surface lands
  (issue #803), since GPU-backed Skia canvases are more likely to support it.
- U+1F601 (😁) maps to a real glyph in this font; `colr_emoji_paints_in_multiple_hues`
  drives that codepoint through the shared `hayate-core` text pipeline.
