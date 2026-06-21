//! ピクセル回帰: scrollbar overlay の thumb（ADR-0110）は content の上、box 端に
//! ラスタライズされ、Scroll Offset を追従し、ネストした scroll-view では外側 box
//! 内にクリップされ続ける。

use hayate_core::{Color, Dimension, ElementId, ElementKind, ElementTree, StyleProp};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn render(tree: &ElementTree, w: u32, h: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(w, h).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);
    pixmap
}

/// overlay の影響を受けない純緑の content: g が高く r/b はほぼ 0。
fn is_plain_green(p: [u8; 4]) -> bool {
    p[1] > 230 && p[0] < 40 && p[2] < 40
}

/// 半透明の暗い thumb が重なって暗くなった緑: 緑優勢のままだが plain な content
/// より明らかに暗い。
fn is_thumb_over_green(p: [u8; 4]) -> bool {
    p[1] > 100 && p[1] < 210 && p[0] < 40 && p[2] < 40
}

fn vertical_scroll_view() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

#[test]
fn thumb_is_painted_at_the_right_edge_over_content() {
    let (tree, _scroll) = vertical_scroll_view();
    let pixmap = render(&tree, 120, 120);

    // 上端近くの右端: ここで thumb が緑の content を暗くする。
    assert!(
        is_thumb_over_green(pixel(&pixmap, 95, 12)),
        "thumb should darken the content at the right edge, got {:?}",
        pixel(&pixmap, 95, 12)
    );
    // thumb の左は plain で全輝度の緑 content。
    assert!(
        is_plain_green(pixel(&pixmap, 40, 12)),
        "content away from the thumb stays plain green, got {:?}",
        pixel(&pixmap, 40, 12)
    );
}

#[test]
fn thumb_tracks_the_offset_in_pixels() {
    let (mut tree, scroll) = vertical_scroll_view();

    // 静止時 thumb は上端近く、track の下端は plain な緑。
    let at_rest = render(&tree, 120, 120);
    assert!(
        is_thumb_over_green(pixel(&at_rest, 95, 12)),
        "thumb starts near the top of the track, got {:?}",
        pixel(&at_rest, 95, 12)
    );
    assert!(
        is_plain_green(pixel(&at_rest, 95, 90)),
        "the bottom of the track is clear at rest, got {:?}",
        pixel(&at_rest, 95, 90)
    );

    // 末尾までスクロールすると thumb は下端へ移動し、上端は晴れる。
    tree.element_set_scroll_offset(scroll, 0.0, 200.0);
    tree.render(0.0);
    let scrolled = render(&tree, 120, 120);
    assert!(
        is_thumb_over_green(pixel(&scrolled, 95, 90)),
        "thumb reaches the bottom after scrolling, got {:?}",
        pixel(&scrolled, 95, 90)
    );
    assert!(
        is_plain_green(pixel(&scrolled, 95, 12)),
        "the top of the track clears after scrolling, got {:?}",
        pixel(&scrolled, 95, 12)
    );
}

fn is_white(p: [u8; 4]) -> bool {
    p == [255, 255, 255, 255]
}

#[test]
fn nested_inner_thumb_stays_inside_the_inner_box() {
    // 外側 140×120 が左上に inner 100×80 scroll-view（緑 100×300 content）を持つ。
    // inner thumb は inner box の右端で content を暗くするが、inner box の下や横の
    // 空いた外側空間には描いてはならない（inner box にクリップ・追従する）。
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let inner = tree.element_create(2, ElementKind::ScrollView);
    let content = tree.element_create(3, ElementKind::View);
    tree.set_root(outer);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(140.0)),
            StyleProp::Height(Dimension::px(120.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, content);
    tree.render(0.0);
    let pixmap = render(&tree, 160, 140);

    // inner thumb は inner box の右端（x≈92..98）で content を暗くする。
    assert!(
        is_thumb_over_green(pixel(&pixmap, 94, 12)),
        "inner thumb paints at the inner box right edge, got {:?}",
        pixel(&pixmap, 94, 12)
    );
    // inner box の下（y 80..120）は空の外側空間: thumb の漏れがあってはならない。
    assert!(
        is_white(pixel(&pixmap, 94, 100)),
        "inner thumb must not leak below the inner box, got {:?}",
        pixel(&pixmap, 94, 100)
    );
    // inner box の右（x 100..140）は空の外側空間: thumb の漏れがあってはならない。
    assert!(
        is_white(pixel(&pixmap, 120, 40)),
        "inner thumb must not leak beside the inner box, got {:?}",
        pixel(&pixmap, 120, 40)
    );
}

#[test]
fn fitting_content_paints_no_thumb() {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    let pixmap = render(&tree, 120, 120);

    // 右端を含む全列が plain な緑 — thumb overlay は出ない。
    assert!(
        is_plain_green(pixel(&pixmap, 95, 50)),
        "a fitting scroll-view paints no thumb at the edge, got {:?}",
        pixel(&pixmap, 95, 50)
    );
}
