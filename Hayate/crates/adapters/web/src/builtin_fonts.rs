//! Web adapter built-in font URL lookup (ADR-0043, ADR-0061).
//!
//! `fonts.json` is the manifest; `build.rs` generates the match table.

include!(concat!(env!("OUT_DIR"), "/builtin_fonts_gen.rs"));

#[cfg(test)]
mod tests {
    use super::builtin_font_url;
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
            Some("https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/inter/Inter%5Bslnt%2Cwght%5D.ttf")
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
    fn unknown_family_returns_none() {
        assert_eq!(builtin_font_url("Comic Sans MS"), None);
        assert_eq!(builtin_font_url(""), None);
    }
}
