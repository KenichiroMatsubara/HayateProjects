//! 回帰防止: CDN 取得のフォールバックフォントが、デフォルトファミリのテキストで
//! バンドル済み日本語フォントを覆い隠してはならない。
//!
//! デプロイ済み Pages で起きた豆腐化を再現・防止する。初回描画は正しく、body フォント
//! (Inter) の取得完了後に □ へ崩れた。旧 register_font は取得した全フォントをデフォルト
//! ファミリ ("Noto Sans") にもエイリアスし、fontique がラン全体にその Latin のみの
//! Inter フェイスを選び、CJK グリフが全て .notdef になっていた。
//!
//! ネイティブ cargo test がこれを捕らえなかった理由は (a) CDN 取得を行わない、
//! (b) FontContext::new() がシステム日本語フォントを読み込み CJK を救済するため。
//! 本テストは WASM 環境 (system_fonts: false) に固定し、実際の登録経路
//! text::register_collection_font を駆動する。

use fontique::{Collection, CollectionOptions, FontInfoOverride, GenericFamily};
use linebender_resource_handle::Blob;
use parley::{FontContext, LayoutContext, PositionedLayoutItem};
use std::sync::Arc;

use hayate_core::element::text::{
    build_text_layout, register_collection_font, DEFAULT_FONT_FAMILY,
};

static NOTO_SANS_JP: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// CDN 取得の Latin フォールバック（デモの body フォント Inter）の代役となる非日本語
/// フォント。DejaVu Sans は CI イメージに存在し、Inter 同様 Latin はカバーするが CJK は
/// カバーしない。
fn latin_fallback_bytes() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

/// WASM 相当の FontContext。システムフォントなし、バンドル JP をデフォルトファミリ +
/// sans-serif generic として登録（layout_pass::init_bundled_fonts と同じ）。
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

    // デモが Latin body フォント (Inter) の取得を終えた状況を、実際の登録経路で再現する。
    register_collection_font(
        &mut font_cx.collection,
        "Inter",
        Arc::new(latin_fallback_bytes()),
    );

    // デモのデフォルトファミリスタックで Latin + CJK を混在させる。
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
