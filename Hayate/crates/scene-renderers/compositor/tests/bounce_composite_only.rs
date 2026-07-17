//! rubber バウンド（overscroll のスプリングバック）を composite-only に保つ work-count 契約
//! （#639・ADR-0125/0127/0131）。#634 の帯内スクロール版（`scroll_composite_only.rs`）をバウンスへ拡張する。
//!
//! 実 `ElementTree` を実バウンス経路（`start_scroll_momentum` → `render` 内 `advance_scroll_motion` →
//! ばね積分 → scroll offset 更新）で 1 フレームずつ駆動し、present 側の帯カバレッジ判定を通す。固定する契約:
//! - スプリングバック中の**全フレームが composite-only**（raster 呼び出し 0 回）。overshoot は合成 affine
//!   （rubber-band translate / Android stretch）が担い、content 帯のピクセルは不変。
//! - scroll offset だけが変わったバウンスフレームは `frame_layer_dirty`（content 再 raster 集合）に scroll
//!   レイヤを載せない（chrome-dirty へ分類・#634）。載るのは合成 transform 変更だけ。
//!
//! ピクセル一致（overshoot 合成が全面 raster と一致）は `layer_scene_parity.rs`。ここは raster **回数**を固定する。

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::layer_scene::{collect_layer_placements, compose};
use hayate_layer_compositor::{
    scroll_content_visible_top, scroll_layer_extent, scroll_layer_geometry, tunables,
    PresentPlanner, ScrollLayerExtent,
};
use std::collections::HashSet;

const VW: f32 = 200.0;
const VH: f32 = 200.0;
const CONTENT_H: f32 = 5000.0;
const BPP: u64 = 4;
const FRAME_MS: f64 = 1000.0 / 60.0;

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
            StyleProp::Height(Dimension::px(VH)),
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

/// present 側が core の幾何から content-visible 帯を組む。**越境中でも `scroll_content_visible_top` で
/// offset を `[0, max]` にクランプ**してから帯を作る（overshoot は合成 affine の担当・#639）。
fn band_for(tree: &ElementTree, scroll: ElementId) -> ScrollLayerExtent {
    let (_, _, _, vh) = tree.element_layout_rect(scroll).unwrap();
    let (_, oy) = tree.element_get_scroll_offset(scroll);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    let top = scroll_content_visible_top(oy, max_y);
    let content_h = vh + max_y;
    scroll_layer_extent(top, vh, content_h, tunables::OVERSCAN_MARGIN_PX)
}

fn band_bytes(tree: &ElementTree, scroll: ElementId) -> u64 {
    let (_, _, w, _) = tree.element_layout_rect(scroll).unwrap();
    let band = band_for(tree, scroll);
    (w as u64) * (band.height.ceil() as u64) * BPP
}

/// 1 フレーム present を回し、raster したか返す。content dirty（子孫内容変化）か content-visible 帯が
/// 可視域を覆っていなければ raster、覆っていれば composite-only（quad transform 平行移動/スケールのみ）。
fn pump(tree: &mut ElementTree, planner: &mut PresentPlanner, scroll: ElementId, ts: f64) -> bool {
    let _ = tree.render(ts);
    let (_, oy) = tree.element_get_scroll_offset(scroll);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    let (_, _, _, vh) = tree.element_layout_rect(scroll).unwrap();
    let top = scroll_content_visible_top(oy, max_y);
    let content_dirty = tree.frame_layer_dirty().contains(&scroll);
    if content_dirty || planner.scroll_layer_needs_raster(scroll, top, vh) {
        planner.note_scroll_rasterized(scroll, band_for(tree, scroll), band_bytes(tree, scroll));
        planner.note_composited(scroll);
        true
    } else {
        planner.note_composited(scroll);
        false
    }
}

/// scroll を端 `edge` の帯までスクロール＆raster してから、`edge + overshoot` で指を離した状態を作る
/// （速度 0・端の外 → スプリングバックが必ず起動）。戻り値はバウンス開始時のクロック。
fn settle_then_release(
    tree: &mut ElementTree,
    planner: &mut PresentPlanner,
    scroll: ElementId,
    to_bottom: bool,
    overshoot_px: f32,
) -> f64 {
    // cold フレーム（top 帯を raster）。ここで初めてレイアウトが確定し max_y が読める。
    assert!(pump(tree, planner, scroll, 0.0), "cold フレームは raster");
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    assert!(max_y > 0.0, "fixture は縦スクロール可能でなければならない");
    let edge = if to_bottom { max_y } else { 0.0 };
    // 端の帯へスクロール（帯を跨ぐので差分 raster する。ここまでは composite-only 対象外）。
    tree.element_set_scroll_offset(scroll, 0.0, edge);
    let _ = pump(tree, planner, scroll, FRAME_MS);
    // 端を越えた位置で解放 → スプリングバック起動。下端は +、上端は - へ越境。
    let release = if to_bottom {
        edge + overshoot_px
    } else {
        edge - overshoot_px
    };
    tree.element_set_scroll_offset(scroll, 0.0, release);
    tree.start_scroll_momentum(scroll, 0.0, 0.0);
    assert!(
        tree.has_pending_visual_work(),
        "overscroll 域での解放はスプリングバックを起動する"
    );
    2.0 * FRAME_MS
}

/// スプリングバックを収束まで駆動し、(フレーム数, raster 回数) を返す。各フレームで content レイヤが
/// `frame_layer_dirty` に載らない（chrome-only 分類・#634/#639）ことも確認する。
fn drive_bounce(
    tree: &mut ElementTree,
    planner: &mut PresentPlanner,
    scroll: ElementId,
    mut ts: f64,
) -> (usize, usize) {
    let mut frames = 0;
    let mut rasters = 0;
    while tree.has_pending_visual_work() && frames < 300 {
        if pump(tree, planner, scroll, ts) {
            rasters += 1;
        }
        assert!(
            !tree.frame_layer_dirty().contains(&scroll),
            "バウンスフレーム {frames}: scroll offset のみの変更が content layer_dirty に載った（合成 transform 変更のはず）"
        );
        frames += 1;
        ts += FRAME_MS;
    }
    (frames, rasters)
}

/// Web/Vello の optimized present が scroll texture に適用する quad affine を再現する。
fn presented_scroll_affine(
    tree: &ElementTree,
    planner: &PresentPlanner,
    scroll: ElementId,
) -> [f64; 6] {
    let root = tree.frame_layers()[0];
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let placement = collect_layer_placements(tree.scene_graph(), root, &boundaries)
        .into_iter()
        .find(|placement| placement.layer == scroll)
        .expect("scroll layer has a placement");
    let geometry = scroll_layer_geometry(tree, scroll).expect("scroll layer has geometry");
    let cached_band = planner
        .cached_scroll_band(scroll)
        .expect("edge band is cached");
    compose(
        placement.transform,
        geometry.composite_affine_for_band(cached_band),
    )
}

#[test]
fn springback_from_bottom_edge_rasters_zero() {
    let (mut tree, scroll) = scroll_tree();
    let mut planner = PresentPlanner::new();
    let ts = settle_then_release(&mut tree, &mut planner, scroll, true, 120.0);
    let (frames, rasters) = drive_bounce(&mut tree, &mut planner, scroll, ts);
    assert!(
        frames >= 10,
        "スプリングバックは複数フレームにわたりアニメーションする（{frames}）"
    );
    assert_eq!(
        rasters, 0,
        "下端バウンスの全 {frames} フレームが composite-only（raster 0）"
    );
}

#[test]
fn springback_from_top_edge_rasters_zero() {
    let (mut tree, scroll) = scroll_tree();
    let mut planner = PresentPlanner::new();
    // 上端（offset 0）は cold 帯がすでに覆う。0 を -120 越えて解放 → 上向きスプリングバック。
    let ts = settle_then_release(&mut tree, &mut planner, scroll, false, 120.0);
    let (frames, rasters) = drive_bounce(&mut tree, &mut planner, scroll, ts);
    assert!(
        frames >= 10,
        "スプリングバックは複数フレームにわたりアニメーションする（{frames}）"
    );
    assert_eq!(
        rasters, 0,
        "上端バウンスの全 {frames} フレームが composite-only（raster 0）"
    );
}

#[test]
fn composite_only_overscroll_changes_the_presented_affine() {
    let (mut tree, scroll) = scroll_tree();
    let mut planner = PresentPlanner::new();

    assert!(pump(&mut tree, &mut planner, scroll, 0.0));
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    tree.element_set_scroll_offset(scroll, 0.0, max_y);
    let _ = pump(&mut tree, &mut planner, scroll, FRAME_MS);
    let edge_core_affine = tree.element_scroll_group_affine(scroll);
    let edge_presented_affine = presented_scroll_affine(&tree, &planner, scroll);

    tree.element_set_scroll_offset(scroll, 0.0, max_y + 120.0);
    let rastered = pump(&mut tree, &mut planner, scroll, 2.0 * FRAME_MS);
    let overscroll_core_affine = tree.element_scroll_group_affine(scroll);
    let overscroll_presented_affine = presented_scroll_affine(&tree, &planner, scroll);

    assert!(
        !rastered,
        "overscroll frame must reuse the cached edge band"
    );
    assert_ne!(
        edge_core_affine, overscroll_core_affine,
        "core must encode the rubber movement in its scroll Group affine"
    );
    assert_ne!(
        edge_presented_affine, overscroll_presented_affine,
        "composite-only present must visibly move the cached texture at the edge"
    );
}

/// #639 の芯の回帰ガード：生 offset（クランプ無し）で帯カバレッジを判定すると、越境フレームは
/// content 帯が可視域を覆えず raster に落ちる。`scroll_content_visible_top` のクランプがそれを消す。
#[test]
fn raw_offset_coverage_would_regress_to_reraster() {
    let (mut tree, scroll) = scroll_tree();
    let _ = tree.render(0.0);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    let (_, _, _, vh) = tree.element_layout_rect(scroll).unwrap();
    let content_h = vh + max_y;

    // 端の帯をキャッシュ相当で用意（content-visible top = max）。
    let cached = scroll_layer_extent(
        scroll_content_visible_top(max_y, max_y),
        vh,
        content_h,
        tunables::OVERSCAN_MARGIN_PX,
    );
    let raw_offset = max_y + 120.0; // バウンスフレーム。

    // 生 offset を可視域上端に使うと覆えない（#634 だけではバウンスが再 raster に落ちる）。
    assert!(
        !cached.covers(raw_offset, vh),
        "生 offset では端の帯が可視域を覆えない"
    );
    // content-visible top（クランプ）なら覆う ＝ composite-only を維持。
    let top = scroll_content_visible_top(raw_offset, max_y);
    assert!(
        cached.covers(top, vh),
        "content-visible top なら端の帯が可視域を覆う"
    );
    let _ = &mut tree;
}
