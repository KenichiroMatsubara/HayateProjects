//! Per-scroll-layer geometry for wiring ADR-0127's overscan-band sizing into `present_layers`
//! (#707).
//!
//! [`ScrollLayerExtent`] (see `lib.rs`) is `LayerCache`/`PresentPlanner`-facing: content-local
//! coordinates (0 = the scroll element's own top edge), used for band-coverage checks and byte
//! bookkeeping. A `LayerRasterizer` needs a different concept to actually place content in a
//! texture — vello has no scissor/viewport render concept (confirmed by inspection:
//! `vello::RenderParams` is just `{base_color, width, height, antialiasing_method}` — width/height
//! *is* the render target, full stop), so the only way to raster a sub-region of a
//! taller-than-viewport scroll layer is to size the destination texture to the band and translate
//! content so the band's own top lands at texture row 0. [`RasterBand`] is that rasterizer-facing
//! type.
//!
//! **Why the band's screen position must be recomputed every frame, not just once at raster
//! time** (this is the crux of composite-only scrolling — #634/ADR-0127's whole premise — and the
//! part that's easy to get wrong): the extracted scene `present_layers` hands the rasterizer
//! already contains the `ScrollView`'s own scroll-offset `Group` transform (`scene_build.rs`'s
//! `scroll_group_affine`, baked in as `translate(-scroll_offset)` — `extract_layer_scene` only
//! strips a *CSS* `transform` group, not this one, since it isn't a direct child of the layer's
//! anchor). So the texture, if rastered with no further adjustment, would show content positioned
//! for *whatever scroll offset was current at raster time* — correct for that one frame, but wrong
//! for any later frame with a different (still in-band) offset, which is exactly the case
//! composite-only reuse depends on. [`ScrollLayerGeometry::screen_top_for_band`] is the one
//! formula behind both directions of this: at raster time it picks the texture-side translate so
//! the texture becomes a *pure content-local snapshot* (row 0 == `band.top`, independent of
//! offset); at composite time (every frame, cached or fresh) it's evaluated again with *this
//! frame's* `visible_top` to find where that snapshot currently belongs on screen. A previous
//! draft of this module computed the raster-time shift as `absolute_top + band.top` (no
//! `visible_top` term) — that's only correct for the exact frame it was rastered on, and silently
//! wrong for every composite-only frame after a scroll within the same band. Caught by hand-tracing
//! the coordinate math against `scene_build.rs`, not by a failing test — there was no test that
//! rastered once and then checked pixels after a *later*, different-offset composite-only frame
//! (see `scroll_overscan_present.rs`'s `banded_present_tracks_further_in_band_scrolling_without_a_reraster`,
//! added once this was found).
//!
//! `ScrollLayerGeometry` also carries the scroll element's own absolute scene position —
//! `ElementTree::element_layout_rect` is absolute (document) coordinates, but scroll offset /
//! content height (and therefore `ScrollLayerExtent`) are content-local (relative to the scroll
//! element's own top edge; see `ElementTree::element_content_size`'s own doc: "値は要素自身の
//! 左上隅を基準とする"). A `ScrollView` that isn't flush with the document's own top edge (the
//! common case — e.g. a header above a scrollable list) needs this to raster/composite the right
//! pixels.
//!
//! [`scroll_layer_geometry_table`] builds one [`ScrollLayerGeometry`] per `ElementKind::ScrollView`
//! layer, once per frame, from `ElementTree` queries — independent of any renderer backend, so
//! `present_layers` (which only sees `&SceneGraph` + layer ids, not `&ElementTree`) can be handed
//! this small table instead of the whole tree.

use std::collections::HashMap;

use hayate_core::{ElementId, ElementKind, ElementTree, ScrollCompositorInput};

use crate::{scroll_content_visible_top, scroll_layer_extent, tunables, ScrollLayerExtent};

/// Where a [`crate::LayerRasterizer`] should render a layer's content and how large to make its
/// cache texture, for a scroll-content band (ADR-0127). `origin_y` is in the same coordinate
/// space as the extracted `SceneGraph` handed to `rasterize` (which already includes the scroll
/// element's own offset `Group` — see this module's doc comment) — it is the shift that turns
/// that scene into a pure content-local snapshot, so the resulting texture's row 0 always holds
/// content-local `band.top`, regardless of scroll position. Content outside `[origin_y, origin_y
/// + height)` simply isn't rendered (vello has no sub-rect/viewport concept; the texture's own
/// extent *is* the render bounds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterBand {
    pub origin_y: f32,
    /// Texture height, logical px (the rasterizer applies its own `content_scale` to convert to
    /// device px, exactly like the existing full-surface path already does for width/height).
    pub height: f32,
}

/// Per-scroll-layer geometry `present_layers` needs each frame to decide whether a scroll
/// content band needs a (re)raster and, if so, at what band (ADR-0127), and to place a texture
/// (fresh or reused) at the correct screen position every frame. Built once per frame by
/// [`scroll_layer_geometry_table`] from `ElementTree` queries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollLayerGeometry {
    /// Content-visible scroll top (`scroll_content_visible_top`-clamped, so overscroll/bounce
    /// frames don't spuriously look uncovered — #639). Content-local: 0 = the scroll element's
    /// own top edge, matching [`ScrollLayerExtent`]'s vocabulary. **This frame's** value — used
    /// every frame (not just when rastering) to place the (possibly reused) band correctly.
    pub visible_top: f32,
    pub viewport_height: f32,
    /// This frame's band, content-local (`PresentPlanner::scroll_layer_needs_raster` /
    /// `note_scroll_rasterized` vocabulary) — the band to record/raster if a (re)raster is
    /// decided (a cache hit does not imply the *cached* band equals this one).
    pub band: ScrollLayerExtent,
    /// The scroll element's own absolute scene top (logical px, from
    /// `ElementTree::element_layout_rect`).
    pub absolute_top: f32,
    /// This layer is in `ElementTree::frame_layer_dirty()` — its **content** (not just scroll
    /// chrome like a scrollbar fade, tracked separately by `frame_layer_chrome_dirty()`) actually
    /// changed this frame, so it must raster even if the cached band still covers the visible
    /// region. Deliberately narrower than the `layer_dirty` callers often pass to
    /// `present_layers` (which merges chrome dirty in too, for backends without scroll-specific
    /// gating) — mirrors `compositor/tests/scroll_composite_only.rs`'s `pump_scroll` exactly,
    /// which checks `frame_layer_dirty()` alone. Using the merged set here would re-raster on
    /// every scrollbar-fade frame, defeating composite-only scrolling.
    pub content_dirty: bool,
}

impl ScrollLayerGeometry {
    /// Where `band.top` (content-local) currently belongs in absolute screen coordinates — the
    /// one formula behind both raster-time translation and composite-time placement (see this
    /// module's doc comment). `band` is usually [`Self::band`] (this frame's fresh band, when
    /// about to raster) or a stale cached band from an earlier raster (when compositing a
    /// composite-only reused texture); either way this uses **this** `ScrollLayerGeometry`'s
    /// `absolute_top`/`visible_top` (i.e. *this frame's*, always) — that's what makes a reused
    /// band still track live scrolling: the band's content-local coordinates don't change once
    /// rastered, only where they currently land on screen does.
    pub fn screen_top_for_band(&self, band: ScrollLayerExtent) -> f32 {
        self.absolute_top + band.top - self.visible_top
    }

    /// The `RasterBand` a `LayerRasterizer` should use to raster [`Self::band`] this frame.
    /// `origin_y` is [`Self::screen_top_for_band`] of `self.band` — the shift that cancels out
    /// both the scroll element's absolute position *and* its current scroll offset (both already
    /// baked into the extracted scene, see the module doc comment), leaving a pure content-local
    /// snapshot.
    pub fn raster_band(&self) -> RasterBand {
        RasterBand {
            origin_y: self.screen_top_for_band(self.band),
            height: self.band.height,
        }
    }
}

/// `layer`'s [`ScrollLayerGeometry`] if it's an `ElementKind::ScrollView` with known layout;
/// `None` for every other layer (present_layers treats those as full-surface, unchanged).
pub fn scroll_layer_geometry(tree: &ElementTree, layer: ElementId) -> Option<ScrollLayerGeometry> {
    if tree.element_kind(layer) != Some(ElementKind::ScrollView) {
        return None;
    }
    let (_, absolute_top, _, viewport_height) = tree.element_layout_rect(layer)?;
    let (_, raw_offset) = tree.element_get_scroll_offset(layer);
    let (_, max_offset) = tree.element_scroll_max_offset(layer);
    let content_height = viewport_height + max_offset;
    let visible_top = scroll_content_visible_top(raw_offset, max_offset);
    let band = scroll_layer_extent(
        visible_top,
        viewport_height,
        content_height,
        tunables::OVERSCAN_MARGIN_PX,
    );
    Some(ScrollLayerGeometry {
        visible_top,
        viewport_height,
        band,
        absolute_top,
        content_dirty: tree.frame_layer_dirty().contains(&layer),
    })
}

/// One [`ScrollLayerGeometry`] per `ElementKind::ScrollView` layer in `layers` (present's frame
/// layer list — typically `ElementTree::frame_layers()`). Non-scroll layers are omitted.
pub fn scroll_layer_geometry_table(
    tree: &ElementTree,
    layers: &[ElementId],
) -> HashMap<ElementId, ScrollLayerGeometry> {
    layers
        .iter()
        .filter_map(|&layer| scroll_layer_geometry(tree, layer).map(|g| (layer, g)))
        .collect()
}

/// Project Core's committed scroll facts into this compositor's overscan policy.
pub fn scroll_layer_geometry_from_inputs(
    inputs: &[ScrollCompositorInput],
) -> HashMap<ElementId, ScrollLayerGeometry> {
    inputs
        .iter()
        .map(|input| {
            let visible_top = scroll_content_visible_top(input.scroll_offset, input.max_scroll_offset);
            let content_height = input.viewport_height + input.max_scroll_offset;
            let band = scroll_layer_extent(
                visible_top,
                input.viewport_height,
                content_height,
                tunables::OVERSCAN_MARGIN_PX,
            );
            (
                input.layer,
                ScrollLayerGeometry {
                    visible_top,
                    viewport_height: input.viewport_height,
                    band,
                    absolute_top: input.absolute_top,
                    content_dirty: input.content_dirty,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::element::style::{Dimension, StyleProp};
    use hayate_core::{Color, DisplayValue, FlexDirectionValue};

    const VW: f32 = 200.0;
    const SCROLL_H: f32 = 200.0;
    const CONTENT_H: f32 = 5000.0;

    fn px(v: f32) -> Dimension {
        Dimension::px(v)
    }

    /// A ScrollView flush with the document's own top edge (root's only child) — the scenario
    /// `compositor/tests/scroll_composite_only.rs` already covers.
    fn scroll_tree_at_document_top() -> (ElementTree, ElementId) {
        let mut tree = ElementTree::new();
        let root = tree.element_create(0, ElementKind::View);
        let scroll = tree.element_create(1, ElementKind::ScrollView);
        let content = tree.element_create(2, ElementKind::View);
        tree.element_append_child(root, scroll);
        tree.element_append_child(scroll, content);
        tree.set_root(root);
        tree.set_viewport(VW, SCROLL_H);
        tree.element_set_style(scroll, &[StyleProp::Width(px(VW)), StyleProp::Height(px(SCROLL_H))]);
        tree.element_set_style(
            content,
            &[
                StyleProp::Width(px(VW)),
                StyleProp::Height(px(CONTENT_H)),
                StyleProp::BackgroundColor(Color::new(0.0, 0.5, 0.0, 1.0)),
            ],
        );
        let _ = tree.render(0.0);
        (tree, scroll)
    }

    /// A ScrollView preceded by a fixed-height header sibling — the common real-world layout
    /// (e.g. the Tsubame todo app's header-above-list) that `scroll_composite_only.rs`'s fixture
    /// does not exercise (its scroll view happens to sit at document-absolute (0,0), which masks
    /// any bug in handling the scroll element's own absolute position).
    const HEADER_H: f32 = 64.0;

    fn scroll_tree_below_header() -> (ElementTree, ElementId) {
        let mut tree = ElementTree::new();
        let root = tree.element_create(0, ElementKind::View);
        let header = tree.element_create(1, ElementKind::View);
        let scroll = tree.element_create(2, ElementKind::ScrollView);
        let content = tree.element_create(3, ElementKind::View);
        tree.element_append_child(root, header);
        tree.element_append_child(root, scroll);
        tree.element_append_child(scroll, content);
        tree.set_root(root);
        tree.set_viewport(VW, HEADER_H + SCROLL_H);
        tree.element_set_style(
            root,
            &[
                StyleProp::Display(DisplayValue::Flex),
                StyleProp::FlexDirection(FlexDirectionValue::Column),
            ],
        );
        tree.element_set_style(header, &[StyleProp::Width(px(VW)), StyleProp::Height(px(HEADER_H))]);
        tree.element_set_style(scroll, &[StyleProp::Width(px(VW)), StyleProp::Height(px(SCROLL_H))]);
        tree.element_set_style(
            content,
            &[
                StyleProp::Width(px(VW)),
                StyleProp::Height(px(CONTENT_H)),
                StyleProp::BackgroundColor(Color::new(0.0, 0.5, 0.0, 1.0)),
            ],
        );
        let _ = tree.render(0.0);
        (tree, scroll)
    }

    #[test]
    fn non_scroll_layer_has_no_geometry() {
        let (tree, _scroll) = scroll_tree_at_document_top();
        let root = ElementId::from_u64(0);
        assert_eq!(scroll_layer_geometry(&tree, root), None, "root is a plain View, not a ScrollView");
    }

    #[test]
    fn scroll_layer_at_document_top_matches_scroll_layer_extent_directly() {
        let (mut tree, scroll) = scroll_tree_at_document_top();
        tree.element_set_scroll_offset(scroll, 0.0, 2000.0);
        let _ = tree.render(16.0);

        let geometry = scroll_layer_geometry(&tree, scroll).expect("scroll view has geometry");
        assert_eq!(geometry.absolute_top, 0.0, "scroll view sits at the document's own top edge");
        assert_eq!(geometry.viewport_height, SCROLL_H);
        assert_eq!(geometry.visible_top, 2000.0, "in-bounds offset needs no #639 clamping");

        let expected = scroll_layer_extent(2000.0, SCROLL_H, CONTENT_H, tunables::OVERSCAN_MARGIN_PX);
        assert_eq!(geometry.band, expected);
        assert!(geometry.band.height < CONTENT_H, "band is not the full content height");
    }

    #[test]
    fn scroll_layer_below_a_header_has_nonzero_absolute_top() {
        let (tree, scroll) = scroll_tree_below_header();
        let geometry = scroll_layer_geometry(&tree, scroll).expect("scroll view has geometry");
        assert_eq!(
            geometry.absolute_top, HEADER_H,
            "scroll view is pushed down by the header's height"
        );
    }

    #[test]
    fn raster_band_composes_absolute_top_band_top_and_visible_top() {
        let (tree, scroll) = scroll_tree_below_header();
        let geometry = scroll_layer_geometry(&tree, scroll).unwrap();
        let raster_band = geometry.raster_band();
        assert_eq!(
            raster_band.origin_y,
            geometry.absolute_top + geometry.band.top - geometry.visible_top,
            "the extracted scene already bakes in the scroll element's own offset Group \
             (scene_build.rs's scroll_group_affine) — origin_y must cancel it out, not just add \
             absolute_top, or the raster would be shifted by the current scroll offset"
        );
        assert_eq!(raster_band.height, geometry.band.height);
    }

    #[test]
    fn raster_band_origin_y_matches_hand_derived_numbers() {
        // Concrete numbers (not just "matches its own formula") to guard against a sign error
        // that could otherwise slip through an algebraic self-consistency check: content
        // 5000px tall, viewport 200, overscan 600 (real tunable), offset 4000 (deep in the list,
        // away from both edges) -> band = [3400, 4800), height 1400.
        let (mut tree, scroll) = scroll_tree_at_document_top();
        tree.element_set_scroll_offset(scroll, 0.0, 4000.0);
        let _ = tree.render(16.0);

        let geometry = scroll_layer_geometry(&tree, scroll).unwrap();
        assert_eq!(geometry.band.top, 3400.0);
        assert_eq!(geometry.band.height, 1400.0);
        assert_eq!(
            geometry.raster_band().origin_y,
            -600.0,
            "abs_y(0.0) + band.top(3400.0) - visible_top(4000.0) == -600.0"
        );
    }

    #[test]
    fn screen_top_for_band_tracks_further_in_band_scrolling_without_a_reraster() {
        // The crux of composite-only scrolling (#634/ADR-0127): raster once at offset 4000
        // (caching band [3400, 4800)), then scroll further to 4050 WITHOUT re-rastering (still
        // covered) — the SAME cached band's on-screen position must shift by exactly the further
        // scroll delta. A formula that only depends on the band (not on *this frame's*
        // visible_top) would freeze the picture instead of tracking the scroll.
        let (mut tree, scroll) = scroll_tree_at_document_top();
        tree.element_set_scroll_offset(scroll, 0.0, 4000.0);
        let _ = tree.render(16.0);
        let at_raster = scroll_layer_geometry(&tree, scroll).unwrap();
        let cached_band = at_raster.band; // what actually gets cached at raster time

        tree.element_set_scroll_offset(scroll, 0.0, 4050.0);
        let _ = tree.render(32.0);
        let later = scroll_layer_geometry(&tree, scroll).unwrap();
        assert!(
            cached_band.covers(later.visible_top, later.viewport_height),
            "test setup: the further scroll must stay within the cached band"
        );

        let position_at_raster = at_raster.screen_top_for_band(cached_band);
        let position_later = later.screen_top_for_band(cached_band);
        assert_eq!(
            position_later,
            position_at_raster - 50.0,
            "the same cached band must move up by exactly the further scroll delta (50px)"
        );
    }

    #[test]
    fn overscrolled_offset_uses_the_639_clamped_visible_top() {
        // Bounce past the bottom edge: raw offset exceeds max_offset. Without the #639 clamp the
        // band would fail to cover the (raw) visible top every bounce frame (see lib.rs's own
        // `overscroll_band_from_visible_top_still_covers_the_viewport` test) — present_layers
        // would re-raster every bounce frame instead of staying composite-only.
        let (mut tree, scroll) = scroll_tree_at_document_top();
        let (_, max_offset) = tree.element_scroll_max_offset(scroll);
        let raw_offset = max_offset + 120.0;
        tree.element_set_scroll_offset(scroll, 0.0, raw_offset);
        let _ = tree.render(16.0);

        let geometry = scroll_layer_geometry(&tree, scroll).unwrap();
        assert_eq!(geometry.visible_top, max_offset, "clamped to the content edge, not the raw overshoot");
        assert!(
            geometry.band.covers(geometry.visible_top, geometry.viewport_height),
            "clamped band must cover the (clamped) visible region"
        );
    }

    #[test]
    fn table_only_contains_scroll_view_layers() {
        let (tree, scroll) = scroll_tree_at_document_top();
        let root = ElementId::from_u64(0);
        let table = scroll_layer_geometry_table(&tree, &[root, scroll]);
        assert_eq!(table.len(), 1);
        assert!(!table.contains_key(&root));
        assert!(table.contains_key(&scroll));
    }

    // ── #707: content_dirty (pure content changes vs. scroll-offset-only frames) ───────────────

    #[test]
    fn scroll_offset_change_alone_is_not_content_dirty() {
        // Pure scrolling (no content mutation) must NOT be content-dirty — this is exactly the
        // case composite-only scrolling exists for. If this were true, `present_layers` would
        // re-raster on every scroll frame regardless of band coverage.
        let (mut tree, scroll) = scroll_tree_at_document_top();
        tree.element_set_scroll_offset(scroll, 0.0, 100.0);
        let _ = tree.render(16.0);
        let geometry = scroll_layer_geometry(&tree, scroll).unwrap();
        assert!(!geometry.content_dirty, "scroll offset alone must not mark content dirty");
    }

    #[test]
    fn content_mutation_is_content_dirty() {
        let (mut tree, scroll) = scroll_tree_at_document_top();
        // Mutate the scrolled content itself (not the scroll offset).
        let content = ElementId::from_u64(2);
        tree.element_set_style(content, &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))]);
        let _ = tree.render(16.0);
        let geometry = scroll_layer_geometry(&tree, scroll).unwrap();
        assert!(geometry.content_dirty, "a content mutation must mark the scroll layer content dirty");
    }
}
