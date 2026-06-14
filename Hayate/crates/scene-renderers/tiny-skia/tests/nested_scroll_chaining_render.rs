//! Pixel regression: nested scroll-view (chaining) clips inner content and
//! keeps the inner Clip box aligned when the outer scroll-view is scrolled
//! (issue #200). Mirrors the CSS Gallery "nested scroll (chaining)" sample shape.

use hayate_core::{
    Color, Dimension, DisplayValue, ElementId, ElementKind, ElementTree, FlexDirectionValue,
    StyleProp,
};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const CANVAS: u32 = 200;

struct NestedScrollFixture {
    tree: ElementTree,
    inner: ElementId,
    tail: ElementId,
    outer_scroll_y: f32,
}

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn is_green(px: [u8; 4]) -> bool {
    px[1] > 200 && px[0] < 80 && px[2] < 80
}

fn is_red(px: [u8; 4]) -> bool {
    px[0] > 200 && px[1] < 80 && px[2] < 80
}

fn is_clear(px: [u8; 4]) -> bool {
    px[0] > 240 && px[1] > 240 && px[2] > 240
}

fn sample_x(rect: (f32, f32, f32, f32)) -> u32 {
    (rect.0 + rect.2 * 0.5) as u32
}

fn painted_y(layout_y: f32, outer_scroll_y: f32) -> u32 {
    (layout_y - outer_scroll_y) as u32
}

/// Same nesting as `CssGallery.tsx` "nested scroll (chaining)": outer ScrollView
/// → column → inner ScrollView (tall green content) → red "outer tail" sibling.
fn nested_scroll_chaining_fixture(outer_scroll_y: f32, with_header: bool) -> NestedScrollFixture {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let column = tree.element_create(2, ElementKind::View);
    let inner = tree.element_create(3, ElementKind::ScrollView);
    let inner_content = tree.element_create(4, ElementKind::View);
    let tail = tree.element_create(5, ElementKind::View);

    tree.set_root(outer);
    tree.set_viewport(CANVAS as f32, CANVAS as f32);

    tree.element_append_child(outer, column);
    if with_header {
        let header = tree.element_create(6, ElementKind::View);
        tree.element_append_child(column, header);
        tree.element_set_style(
            header,
            &[
                StyleProp::Width(Dimension::px(160.0)),
                StyleProp::Height(Dimension::px(40.0)),
                StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            ],
        );
    }
    tree.element_append_child(column, inner);
    tree.element_append_child(inner, inner_content);
    tree.element_append_child(column, tail);

    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(180.0)),
            StyleProp::Height(Dimension::px(120.0)),
        ],
    );
    tree.element_set_style(
        column,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(8.0)),
            StyleProp::Width(Dimension::px(180.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(64.0)),
        ],
    );
    tree.element_set_style(
        inner_content,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style(
        tail,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(24.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );

    if outer_scroll_y != 0.0 {
        tree.element_set_scroll_offset(outer, 0.0, outer_scroll_y);
    }
    tree.render(0.0);

    NestedScrollFixture {
        tree,
        inner,
        tail,
        outer_scroll_y,
    }
}

fn render(fixture: &NestedScrollFixture) -> Pixmap {
    let mut pixmap = Pixmap::new(CANVAS, CANVAS).unwrap();
    TinySkiaSceneRenderer::new().render_scene(fixture.tree.scene_graph(), &mut pixmap, CLEAR, 1.0);
    pixmap
}

#[test]
fn nested_scroll_inner_content_does_not_bleed_over_outer_tail() {
    let fixture = nested_scroll_chaining_fixture(0.0, false);
    let pixmap = render(&fixture);

    let inner = fixture
        .tree
        .element_layout_rect(fixture.inner)
        .expect("inner layout");
    let tail = fixture
        .tree
        .element_layout_rect(fixture.tail)
        .expect("tail layout");
    let x = sample_x(inner);

    assert!(
        is_green(pixel(
            &pixmap,
            x,
            painted_y(inner.1 + inner.3 * 0.25, fixture.outer_scroll_y)
        )),
        "inner scroll-view should show green content inside its box"
    );
    assert!(
        is_clear(pixel(
            &pixmap,
            x,
            painted_y(inner.1 + inner.3 + 4.0, fixture.outer_scroll_y)
        )),
        "inner overflow must be clipped below the inner box (got {:?})",
        pixel(
            &pixmap,
            x,
            painted_y(inner.1 + inner.3 + 4.0, fixture.outer_scroll_y)
        )
    );
    assert!(
        is_red(pixel(
            &pixmap,
            x,
            painted_y(tail.1 + tail.3 * 0.5, fixture.outer_scroll_y)
        )),
        "outer tail must paint red without inner green overlap (got {:?})",
        pixel(
            &pixmap,
            x,
            painted_y(tail.1 + tail.3 * 0.5, fixture.outer_scroll_y)
        )
    );
}

#[test]
fn nested_scroll_inner_clip_tracks_outer_scroll_offset() {
    let fixture = nested_scroll_chaining_fixture(20.0, true);
    let pixmap = render(&fixture);

    let inner = fixture
        .tree
        .element_layout_rect(fixture.inner)
        .expect("inner layout");
    let tail = fixture
        .tree
        .element_layout_rect(fixture.tail)
        .expect("tail layout");
    let x = sample_x(inner);

    assert!(
        is_green(pixel(
            &pixmap,
            x,
            painted_y(inner.1 + inner.3 * 0.25, fixture.outer_scroll_y)
        )),
        "scrolled outer: green should remain inside the inner box (got {:?})",
        pixel(
            &pixmap,
            x,
            painted_y(inner.1 + inner.3 * 0.25, fixture.outer_scroll_y)
        )
    );
    assert!(
        is_clear(pixel(
            &pixmap,
            x,
            painted_y(inner.1 + inner.3 + 4.0, fixture.outer_scroll_y)
        )),
        "scrolled outer: inner overflow must stay clipped (got {:?})",
        pixel(
            &pixmap,
            x,
            painted_y(inner.1 + inner.3 + 4.0, fixture.outer_scroll_y)
        )
    );
    assert!(
        is_red(pixel(
            &pixmap,
            x,
            painted_y(tail.1 + tail.3 * 0.5, fixture.outer_scroll_y)
        )),
        "scrolled outer: tail must stay visible and un-overlapped (got {:?})",
        pixel(
            &pixmap,
            x,
            painted_y(tail.1 + tail.3 * 0.5, fixture.outer_scroll_y)
        )
    );
}
