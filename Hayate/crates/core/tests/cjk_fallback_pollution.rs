//! Regression: a CDN-fetched fallback font must NOT shadow the bundled Japanese
//! font for default-family text.
//!
//! Reproduces (and now guards against) the deployed-Pages tofu *cascade*: text
//! rendered correctly on first paint, then collapsed to □ once the body font
//! (Inter) finished fetching. The old `register_font` aliased every fetched font
//! ALSO under the default family ("Noto Sans"); fontique then selected that
//! Latin-only Inter face for whole runs, so every CJK glyph became `.notdef`.
//!
//! Native `cargo test` never caught this because (a) it never performs the CDN
//! fetch and (b) `FontContext::new()` loads system Japanese fonts that rescue the
//! CJK. This test pins the WASM environment (`system_fonts: false`) and drives
//! the REAL registration path, `text::register_collection_font`.

use fontique::{Collection, CollectionOptions, FontInfoOverride, GenericFamily};
use linebender_resource_handle::Blob;
use parley::{FontContext, LayoutContext, PositionedLayoutItem};
use std::sync::Arc;

use hayate_core::element::text::{
    build_text_layout, register_collection_font, DEFAULT_FONT_FAMILY,
};

static NOTO_SANS_JP: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// A non-Japanese font standing in for a CDN-fetched Latin fallback (Inter, the
/// demo's body font). DejaVu Sans is present on the CI image and, like Inter,
/// covers Latin but not CJK.
fn latin_fallback_bytes() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

/// WASM-like FontContext: no system fonts; bundled JP registered as the default
/// family + sans-serif generic (mirrors `layout_pass::init_bundled_fonts`).
fn wasm_like_font_context() -> FontContext {
    let mut font_cx = FontContext::default();
    font_cx.collection = Collection::new(CollectionOptions {
        system_fonts: false,
        ..Default::default()
    });
    let jp = Arc::new(NOTO_SANS_JP.to_vec());
    let override_info = FontInfoOverride {
        family_name: Some(DEFAULT_FONT_FAMILY),
        ..Default::default()
    };
    let registered = font_cx
        .collection
        .register_fonts(Blob::new(jp), Some(override_info));
    let ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
    font_cx
        .collection
        .set_generic_families(GenericFamily::SansSerif, ids.into_iter());
    font_cx
}

fn glyph_ids(layout: &parley::Layout<[u8; 4]>) -> Vec<u32> {
    layout
        .lines()
        .flat_map(|line| line.items())
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(grun) => Some(grun),
            _ => None,
        })
        .flat_map(|grun| grun.glyphs().map(|g| g.id).collect::<Vec<_>>())
        .collect()
}

#[test]
fn fetched_fallback_font_does_not_shadow_bundled_japanese() {
    let mut font_cx = wasm_like_font_context();

    // Simulate the demo finishing the fetch of its Latin body font (Inter),
    // through the real registration path.
    register_collection_font(
        &mut font_cx.collection,
        "Inter",
        Arc::new(latin_fallback_bytes()),
    );

    // Mixed Latin + CJK with the demo's default-family stack.
    let mut layout_cx = LayoutContext::new();
    let tl = build_text_layout(
        &mut font_cx,
        &mut layout_cx,
        "block きょうのタスク 一二三",
        16.0,
        None,
        Some("Inter, Segoe UI, system-ui, sans-serif"),
        None,
        None,
    );
    let ids = glyph_ids(&tl.layout);
    assert!(!ids.is_empty(), "no glyphs shaped");
    assert!(
        ids.iter().all(|&id| id != 0),
        "Japanese shaped to .notdef (tofu) after a Latin fallback was fetched: {ids:?}"
    );
}
