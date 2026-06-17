# Test font provenance

## `colr_test_glyphs.ttf`

- **Source**: [`googlefonts/color-fonts`](https://github.com/googlefonts/color-fonts),
  file `fonts/test_glyphs-glyf_colr_1.ttf`.
- **License**: Apache License 2.0 (see the upstream repository's `LICENSE`).
- **Purpose**: a tiny (~21 KB) COLRv1 + CPAL font used to prove that the Vello
  backend paints colour glyphs through `try_draw_colr` (issue #332). Its glyphs
  are gradient/sweep test shapes mapped into the PUA (`U+F0100`…), and its
  palette is a rainbow, so a correctly painted glyph yields several distinct
  hues. A monochrome painter (or a backend that ignores COLR) would render a
  single ink colour.

This is a **test-only** asset; it is not shipped and not referenced by the
production font manifest (`crates/adapters/web/fonts.json`).
