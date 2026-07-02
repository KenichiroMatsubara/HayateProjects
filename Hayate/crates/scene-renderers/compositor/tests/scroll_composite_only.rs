//! scroll レイヤ composite-only スクロールの work-count 契約（#634・ADR-0127）。
//!
//! 実 `ElementTree` を per-layer present 経路（`plan_scroll` → 帯カバレッジ判定）で駆動したとき:
//! - overscan 帯**内**のスクロールフレームは raster 0 回（quad の平行移動のみ）
//! - 帯を**外れた**スクロールで新帯が差分 raster され、可視域が常にカバーされる
//! - scroll レイヤ texture のバイトは帯サイズ（可視域＋overscan）で計上し、content 全高で確保しない
//! - GPU 予算超過で最も長く composite に使われていないレイヤから LRU 退避される
//!
//! altitude は #633 の `transform_composite_only.rs` と同じ planner seam：raster **回数**を固定する。
//! ピクセル一致（scroll offset を quad で適用しても全面 raster と一致）は `layer_scene_parity.rs`。

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::{
    scroll_layer_extent, tunables, GpuBudget, PresentPlanner, ScrollLayerExtent,
};

const VW: f32 = 200.0;
const VH: f32 = 200.0;
/// scroll ビューポート（content より小さい）。
const SCROLL_H: f32 = 200.0;
const CONTENT_H: f32 = 5000.0;
/// 4 バイト/px（RGBA8）。
const BPP: u64 = 4;

/// content 300... ではなく十分高い ScrollView を 1 つ持つツリー。
fn scroll_tree() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(VW, VH);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(SCROLL_H)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(VW)),
            StyleProp::Height(Dimension::px(CONTENT_H)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.5, 0.0, 1.0)),
        ],
    );
    (tree, scroll)
}

/// present 側が core の幾何から帯を組む（`scroll_layer_extent`）。可視域上端 = scroll offset。
fn band_for(tree: &ElementTree, scroll: ElementId) -> ScrollLayerExtent {
    let (_, _, _, vh) = tree.element_layout_rect(scroll).unwrap();
    let (_, oy) = tree.element_get_scroll_offset(scroll);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    let content_h = vh + max_y;
    scroll_layer_extent(oy, vh, content_h, tunables::OVERSCAN_MARGIN_PX)
}

/// 帯サイズのキャッシュ texture バイト（幅 × 帯高 × 4）。全高分は確保しない。
fn band_bytes(tree: &ElementTree, scroll: ElementId) -> u64 {
    let (_, _, w, _) = tree.element_layout_rect(scroll).unwrap();
    let band = band_for(tree, scroll);
    (w as u64) * (band.height.ceil() as u64) * BPP
}

/// scroll レイヤ present を 1 フレーム回し、raster したかどうかを返す。帯がまだ可視域を覆って
/// いれば raster せず（composite-only）、外れていれば帯を差分 raster してキャッシュを更新する。
fn pump_scroll(tree: &mut ElementTree, planner: &mut PresentPlanner, scroll: ElementId, ts: f64) -> bool {
    let _ = tree.render(ts);
    let (_, oy) = tree.element_get_scroll_offset(scroll);
    let (_, _, _, vh) = tree.element_layout_rect(scroll).unwrap();
    // content dirty（子孫の内容変化）なら常に raster。scroll offset だけの変化は content dirty に
    // ならない（chrome dirty）ので、帯カバレッジだけが raster 要否を決める。
    let content_dirty = tree.frame_layer_dirty().contains(&scroll);
    if content_dirty || planner.scroll_layer_needs_raster(scroll, oy, vh) {
        planner.note_scroll_rasterized(scroll, band_for(tree, scroll), band_bytes(tree, scroll));
        planner.note_composited(scroll);
        true
    } else {
        planner.note_composited(scroll);
        false
    }
}

#[test]
fn scroll_within_overscan_band_rasters_zero() {
    let (mut tree, scroll) = scroll_tree();
    let mut planner = PresentPlanner::new();
    // cold フレーム：初回は帯を raster する。
    assert!(pump_scroll(&mut tree, &mut planner, scroll, 0.0), "cold フレームは raster");

    // overscan 帯内の小さなスクロール（overscan = 600px なので 100px ずつは帯内）。
    for frame in 1..=5 {
        tree.element_set_scroll_offset(scroll, 0.0, frame as f32 * 100.0);
        let rastered = pump_scroll(&mut tree, &mut planner, scroll, frame as f64 * 16.0);
        assert!(!rastered, "帯内スクロールのフレーム {frame} で raster が走った（composite-only 違反）");
    }
}

#[test]
fn scrolling_past_the_band_rerasters_and_recovers_coverage() {
    let (mut tree, scroll) = scroll_tree();
    let mut planner = PresentPlanner::new();
    let _ = pump_scroll(&mut tree, &mut planner, scroll, 0.0);

    // 帯（可視域 + overscan）を確実に外れる大ジャンプ。
    let jump = SCROLL_H + tunables::OVERSCAN_MARGIN_PX + 100.0;
    tree.element_set_scroll_offset(scroll, 0.0, jump);
    let rastered = pump_scroll(&mut tree, &mut planner, scroll, 16.0);
    assert!(rastered, "帯を外れたスクロールは差分 raster する");

    // 新帯は可視域を覆う。
    let (_, _, _, vh) = tree.element_layout_rect(scroll).unwrap();
    let band = band_for(&tree, scroll);
    assert!(band.covers(jump, vh), "差分 raster 後の新帯が可視域をカバーする");
}

#[test]
fn scroll_layer_texture_is_band_sized_not_full_content() {
    let (mut tree, scroll) = scroll_tree();
    let mut planner = PresentPlanner::new();
    let _ = pump_scroll(&mut tree, &mut planner, scroll, 0.0);

    let (_, _, w, vh) = tree.element_layout_rect(scroll).unwrap();
    let full_content_bytes = (w as u64) * (CONTENT_H.ceil() as u64) * BPP;
    // 帯高は 可視域(vh) + overscan(上下) を content 全高でクランプした値。全高よりはるかに小さい。
    let band = band_for(&tree, scroll);
    assert!(band.height < CONTENT_H, "帯高は content 全高より小さい");
    assert_eq!(
        planner.cached_bytes(),
        band_bytes(&tree, scroll),
        "キャッシュバイトは帯サイズで計上される"
    );
    assert!(planner.cached_bytes() < full_content_bytes, "content 全高分は確保しない");
    let _ = vh;
}

#[test]
fn over_budget_evicts_least_recently_composited_scroll_layer() {
    // 2 つの scroll レイヤを raster し、layer A を最近 composite → 予算超過で B（LRU）が退避される。
    let mut planner = PresentPlanner::new();
    let a = ElementId::from_u64(1);
    let b = ElementId::from_u64(2);
    let extent = ScrollLayerExtent { top: 0.0, height: 100.0 };
    planner.note_scroll_rasterized(a, extent, 1000);
    planner.note_scroll_rasterized(b, extent, 1000);
    // A を最近 composite（B は古いまま）。
    planner.note_composited(b);
    planner.note_composited(a);

    // 予算 = 1 枚分だけ → LRU（B）が退避される。
    let evicted = planner.enforce_budget(GpuBudget::from_bytes(1000));
    assert_eq!(evicted, vec![b], "最も長く composite に使われていない B が退避される");
    assert!(planner.cached_bytes() <= 1000, "退避後は合計が予算内");
    // 退避されたレイヤは次フレーム未キャッシュ扱い＝再 raster が要る。
    assert!(planner.scroll_layer_needs_raster(b, 0.0, 100.0), "退避レイヤは再 raster 対象");
    assert!(!planner.scroll_layer_needs_raster(a, 0.0, 100.0), "残ったレイヤは帯カバーで composite-only");
}
