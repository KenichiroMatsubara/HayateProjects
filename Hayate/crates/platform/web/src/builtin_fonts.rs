//! web アダプタの組み込みフォント URL ルックアップ（ADR-0043, ADR-0061）。
//!
//! `fonts.json` がマニフェスト。`build.rs` がマッチ表を生成する。

include!(concat!(env!("OUT_DIR"), "/builtin_fonts_gen.rs"));

#[cfg(any(target_arch = "wasm32", test))]
use hayate_app_host::renderer_selection::SceneRendererKind;

/// カラー（COLR/CPAL）emoji ビルド。カラーグリフを描けるレンダラのみここへ
/// 振り分ける。バイト列は core が要求したファミリ名で登録されるため、core の
/// コードポイント→ファミリ振り分け（ADR-0101）には影響しない。モノクロ側の
/// ファミリ識別子と「格上げするか」の規則は core 所有（[`hayate_core::element::font_coverage`]）で、
/// ここでは複製しない。
#[cfg(any(target_arch = "wasm32", test))]
const COLOR_EMOJI_FAMILY: &str = "Noto Color Emoji";

/// レンダラを考慮したフォント調達（ADR-0043）。core が `.notdef` コードポイントを
/// 振り分けたファミリと有効なレンダラを受け取り、fetch する URL を返す。emoji
/// フォールバック以外は [`builtin_font_url`] と同一。カラーグリフを描けるレンダラ
/// （Vello）では、モノクロ emoji ファミリをカラービルドへ格上げする（格上げ規則は
/// core の `upgrades_to_color_emoji`）。fetch したバイト列は core が要求したファミリ名で
/// 登録されるため、このディスパッチは core のコードポイント→ファミリ表からは見えない。
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn font_url_for_renderer(
    family: &str,
    renderer: SceneRendererKind,
) -> Option<&'static str> {
    if hayate_core::element::font_coverage::upgrades_to_color_emoji(
        family,
        renderer.paints_color_glyphs(),
    ) {
        return builtin_font_url(COLOR_EMOJI_FAMILY);
    }
    builtin_font_url(family)
}

#[cfg(test)]
mod tests {
    use super::{builtin_font_url, font_url_for_renderer};
    use hayate_app_host::renderer_selection::SceneRendererKind;
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
        // レイヤ間整合性（ADR-0101）: core のカバレッジ表が .notdef コードポイント
        // を振り分けうる全フォールバックファミリは、このアダプタのマニフェストに
        // 供給元を持たねばならない。さもなくば FetchFont が行き止まる。これにより
        // カバレッジ表をデータだけで安全に拡張できる。
        for family in hayate_core::element::font_coverage::coverage_families() {
            assert!(
                builtin_font_url(family).is_some(),
                "coverage family {family:?} has no URL in fonts.json"
            );
        }
    }

    #[test]
    fn emoji_fallback_resolves_to_monochrome_noto_emoji() {
        // hayate-core は emoji コードポイントをファミリ "Noto Emoji" に対応づける。
        // マニフェストはそれをモノクロビルドに解決せねばならない（tiny-skia は
        // COLR/CBDT のカラーグリフを描けない）。
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
        // Vello（WebGPU）は COLR/CPAL を描けるので、core が振り分ける emoji ファミリ
        // ("Noto Emoji", ADR-0101) を GPU 経路ではカラービルドへ格上げする。
        // ディスパッチはこのアダプタに留まり（ADR-0043）、core のコードポイント→
        // ファミリ表は不変。
        let url = font_url_for_renderer("Noto Emoji", SceneRendererKind::Vello)
            .expect("emoji family must resolve on Vello");
        assert!(
            url.to_lowercase().contains("notocoloremoji"),
            "Vello must fetch the colour Noto Color Emoji build, got {url}"
        );
    }

    #[test]
    fn tiny_skia_keeps_monochrome_emoji() {
        // CPU フォールバックは COLR/CBDT を描けないので、emoji ファミリはモノクロ
        // ビルドに留めねばならない（カラー emoji への退行を防ぐ。ADR-0101）。
        let url = font_url_for_renderer("Noto Emoji", SceneRendererKind::TinySkia)
            .expect("emoji family must resolve on tiny-skia");
        let lower = url.to_lowercase();
        assert!(lower.contains("notoemoji"), "expected Noto Emoji, got {url}");
        assert!(
            !lower.contains("color"),
            "tiny-skia must stay monochrome, not a colour build: {url}"
        );
        // レンダラ非依存のルックアップと同一: CPU レンダラは何も加えない。
        assert_eq!(
            font_url_for_renderer("Noto Emoji", SceneRendererKind::TinySkia),
            builtin_font_url("Noto Emoji"),
        );
    }

    #[test]
    fn non_emoji_families_are_renderer_independent() {
        // レンダラ考慮のディスパッチは emoji フォールバックにのみ作用する。他の
        // ファミリはどのレンダラでも同一に解決される。
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
        // カラービルドはモノクロと並んでマニフェストに存在し、Noto Color Emoji
        // (COLR) の URL に解決され、別ファイルである。
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
