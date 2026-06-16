//! Regression: a CDN-fetched fallback family (e.g. Noto Emoji) must NOT shadow
//! the bundled Japanese font for default-family text.
//!
//! Reproduces the deployed-Pages tofu: `Tree::register_font` registers every
//! fetched font ALSO under the default family ("Noto Sans") so it works without
//! an explicit `font-family`. But that means fetching a non-Japanese fallback
//! (Noto Emoji, triggered by the demo's 🌙 emoji) adds a non-JP face to the
//! "Noto Sans" family, which fontique then selects for the whole Japanese run
//! → `.notdef` (glyph 0) → tofu boxes.
//!
//! Native `cargo test` never hit this because (a) it never performs the CDN
//! fetch and (b) `FontContext::new()` loads system Japanese fonts that rescue
//! the CJK. This test pins the WASM environment: `system_fonts: false`, and
//! mimics the fetch via raw collection registration matching `register_font`.

use fontique::{Collection, CollectionOptions, FontInfoOverride, GenericFamily};
use linebender_resource_handle::Blob;
use parley::{FontContext, LayoutContext, PositionedLayoutItem};
use std::sync::Arc;

use hayate_core::element::text::{build_text_layout, TextBrush, DEFAULT_FONT_FAMILY};

static NOTO_SANS_JP: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// A non-Japanese font standing in for a CDN-fetched fallback (Noto Emoji has
/// no CJK either). DejaVu Sans is present on the CI image.
fn non_jp_fallback_bytes() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

fn register(collection: &mut Collection, family: &str, bytes: Arc<Vec<u8>>) {
    let override_info = FontInfoOverride {
        family_name: Some(family),
        ..Default::default()
    };
    collection.register_fonts(Blob::new(bytes), Some(override_info));
}

/// WASM-like FontContext: no system fonts, bundled JP registered as the default
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

fn glyph_ids(layout: &parley::Layout<TextBrush>) -> Vec<u32> {
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
#[ignore = "reproduces the deployed-Pages CJK tofu (register_font aliases a \
fetched fallback under the default family, shadowing the bundled JP face). \
Remove #[ignore] together with the register_font fix."]
fn fetched_fallback_family_does_not_shadow_bundled_japanese() {
    let mut font_cx = wasm_like_font_context();

    // Simulate the demo fetching a non-JP fallback (Noto Emoji) and the current
    // `register_font` behaviour: register it under its own name AND, because it
    // is not the default family, ALSO under DEFAULT_FONT_FAMILY ("Noto Sans").
    let fallback = Arc::new(non_jp_fallback_bytes());
    register(&mut font_cx.collection, "Noto Emoji", fallback.clone());
    register(&mut font_cx.collection, DEFAULT_FONT_FAMILY, fallback);

    let mut layout_cx = LayoutContext::new();
    let tl = build_text_layout(
        &mut font_cx,
        &mut layout_cx,
        "きょうのタスク一二三",
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
        "Japanese shaped to .notdef (tofu) after a non-JP fallback was fetched: {ids:?}"
    );
}
