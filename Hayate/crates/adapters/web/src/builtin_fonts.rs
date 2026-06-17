//! Web adapter built-in font URL lookup (ADR-0043, ADR-0061).
//!
//! `fonts.json` is the manifest; `build.rs` generates the match table.

include!(concat!(env!("OUT_DIR"), "/builtin_fonts_gen.rs"));

#[cfg(any(target_arch = "wasm32", test))]
use crate::renderer_selection::SceneRendererKind;

/// The monochrome emoji family core's coverage table routes `.notdef` emoji
/// codepoints to (ADR-0101). Renderer-independent; the lowest common
/// denominator both painters can draw.
const MONOCHROME_EMOJI_FAMILY: &str = "Noto Emoji";

/// The colour (COLR/CPAL) emoji build. Only renderers that can paint colour
/// glyphs are routed here; the bytes are still registered under the family core
/// asked for, so core's codepoint→family routing (ADR-0101) is untouched.
const COLOR_EMOJI_FAMILY: &str = "Noto Color Emoji";

/// Renderer-aware font procurement (ADR-0043, #332): given the family core
/// routed a `.notdef` codepoint to and the active renderer, return the URL to
/// fetch. Identical to [`builtin_font_url`] for every family except the emoji
/// fallback: on a renderer that paints colour glyphs (Vello), the monochrome
/// emoji family is upgraded to the colour build. The fetched bytes are
/// registered under the family core requested, so this dispatch is invisible to
/// core's codepoint→family table.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn font_url_for_renderer(
    family: &str,
    renderer: SceneRendererKind,
) -> Option<&'static str> {
    if family == MONOCHROME_EMOJI_FAMILY && renderer.paints_color_glyphs() {
        return builtin_font_url(COLOR_EMOJI_FAMILY);
    }
    builtin_font_url(family)
}

#[cfg(test)]
mod tests {
    use super::{builtin_font_url, font_url_for_renderer};
    use crate::renderer_selection::SceneRendererKind;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn fonts_manifest() -> Vec<serde_json::Value> {
        let text = fs::read_to_string(manifest_dir().join("fonts.json")).expect("read fonts.json");
        serde_json::from_str(&text).expect("parse fonts.json")
    }

    fn font_family_enum_values() -> Vec<String> {
        let enums_path = manifest_dir().join("../../../proto/spec/enums.json");
        let text = fs::read_to_string(&enums_path).expect("read enums.json");
        let enums: Vec<serde_json::Value> = serde_json::from_str(&text).expect("parse enums.json");
        let font_family = enums
            .iter()
            .find(|e| e["name"] == "font_family")
            .expect("font_family enum");
        font_family["values"]
            .as_array()
            .expect("font_family values")
            .iter()
            .map(|v| v["value"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn every_font_family_preset_has_url() {
        let manifest = fonts_manifest();
        let urls: HashMap<String, String> = manifest
            .iter()
            .map(|entry| {
                (
                    entry["family"].as_str().unwrap().to_string(),
                    entry["url"].as_str().unwrap().to_string(),
                )
            })
            .collect();

        for family in font_family_enum_values() {
            assert!(
                urls.contains_key(&family),
                "font_family preset {family:?} missing from fonts.json"
            );
        }
    }

    #[test]
    fn known_families_return_expected_urls() {
        assert_eq!(
            builtin_font_url("Noto Sans JP"),
            Some("https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf")
        );
        assert_eq!(
            builtin_font_url("Inter"),
            Some("https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/inter/Inter%5Bopsz%2Cwght%5D.ttf")
        );
        assert_eq!(
            builtin_font_url("Lato"),
            Some("https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/lato/Lato-Regular.ttf")
        );
        assert_eq!(
            builtin_font_url("M PLUS Rounded 1c"),
            Some("https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/mplusrounded1c/MPLUSRounded1c-Regular.ttf")
        );
    }

    #[test]
    fn every_coverage_family_is_procurable() {
        // Cross-layer integrity (ADR-0101): every fallback family the core
        // coverage table can route a .notdef codepoint to MUST have a source in
        // this adapter's manifest, or the FetchFont would dead-end. This is what
        // makes the coverage table safe to extend by data alone.
        for family in hayate_core::element::font_coverage::coverage_families() {
            assert!(
                builtin_font_url(family).is_some(),
                "coverage family {family:?} has no URL in fonts.json"
            );
        }
    }

    #[test]
    fn emoji_fallback_resolves_to_monochrome_noto_emoji() {
        // hayate-core maps emoji codepoints to the family "Noto Emoji"; the
        // manifest must resolve it to the MONOCHROME build (tiny-skia cannot
        // paint COLR/CBDT colour glyphs — issue #329).
        let url = builtin_font_url("Noto Emoji").expect("Noto Emoji missing from fonts.json");
        let lower = url.to_lowercase();
        assert!(
            lower.contains("notoemoji"),
            "expected a Noto Emoji url, got {url}"
        );
        assert!(
            !lower.contains("color"),
            "fallback must be monochrome Noto Emoji, not a colour build: {url}"
        );
    }

    #[test]
    fn vello_routes_emoji_family_to_color_build() {
        // Vello (WebGPU) paints COLR/CPAL, so the emoji family core routes to
        // ("Noto Emoji", ADR-0101) is upgraded to the colour build on the GPU
        // path (#332). The dispatch stays in this adapter (ADR-0043); core's
        // codepoint→family table is untouched.
        let url = font_url_for_renderer("Noto Emoji", SceneRendererKind::Vello)
            .expect("emoji family must resolve on Vello");
        assert!(
            url.to_lowercase().contains("notocoloremoji"),
            "Vello must fetch the colour Noto Color Emoji build, got {url}"
        );
    }

    #[test]
    fn tiny_skia_keeps_monochrome_emoji() {
        // CPU fallback cannot paint COLR/CBDT, so the emoji family must stay on
        // the monochrome build — no colour-emoji regression (#332 AC, ADR-0101).
        let url = font_url_for_renderer("Noto Emoji", SceneRendererKind::TinySkia)
            .expect("emoji family must resolve on tiny-skia");
        let lower = url.to_lowercase();
        assert!(lower.contains("notoemoji"), "expected Noto Emoji, got {url}");
        assert!(
            !lower.contains("color"),
            "tiny-skia must stay monochrome, not a colour build: {url}"
        );
        // Identical to the renderer-agnostic lookup: CPU renderers add nothing.
        assert_eq!(
            font_url_for_renderer("Noto Emoji", SceneRendererKind::TinySkia),
            builtin_font_url("Noto Emoji"),
        );
    }

    #[test]
    fn non_emoji_families_are_renderer_independent() {
        // The renderer-aware dispatch only touches the emoji fallback; every
        // other family resolves identically on every renderer.
        for family in ["Noto Sans JP", "Inter", "Noto Sans Arabic"] {
            for renderer in [SceneRendererKind::Vello, SceneRendererKind::TinySkia] {
                assert_eq!(
                    font_url_for_renderer(family, renderer),
                    builtin_font_url(family),
                    "{family} must not change with renderer {renderer:?}"
                );
            }
        }
    }

    #[test]
    fn color_emoji_build_is_procurable_and_distinct() {
        // The colour build lives in the manifest alongside the monochrome one,
        // resolves to the Noto Color Emoji (COLR) URL, and is a different file.
        let color =
            builtin_font_url("Noto Color Emoji").expect("color build missing from manifest");
        let mono = builtin_font_url("Noto Emoji").expect("mono build missing from manifest");
        assert!(
            color.to_lowercase().contains("notocoloremoji"),
            "expected a Noto Color Emoji url, got {color}"
        );
        assert_ne!(color, mono, "color and monochrome builds must be distinct urls");
    }

    #[test]
    fn unknown_family_returns_none() {
        assert_eq!(builtin_font_url("Comic Sans MS"), None);
        assert_eq!(builtin_font_url(""), None);
    }
}
