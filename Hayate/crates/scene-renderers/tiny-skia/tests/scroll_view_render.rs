//! ピクセル回帰: ScrollView の内容は自分のボックスにクリップされねばならない。

use hayate_core::{Color, Dimension, ElementKind, ElementTree, StyleProp};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn scroll_view_with_tall_content(height: f32) -> ElementTree {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(height)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    tree
}

#[test]
fn scroll_view_clips_overflowing_content() {
    let tree = scroll_view_with_tall_content(50.0);
    let mut pixmap = Pixmap::new(120, 120).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);

    assert!(
        pixel(&pixmap, 50, 25)[1] > 200,
        "content inside the 50px viewport should be visible"
    );
    assert_eq!(
        pixel(&pixmap, 50, 80),
        [255, 255, 255, 255],
        "overflow content must be clipped to the scroll-view box"
    );
}

/// CSS Gallery の単一 `scroll-view` サンプル（height: 72、内容はそれより高い）を再現する。
#[test]
fn css_gallery_scroll_view_height_clips_overflow() {
    let tree = scroll_view_with_tall_content(72.0);
    let mut pixmap = Pixmap::new(120, 120).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);

    assert!(
        pixel(&pixmap, 50, 36)[1] > 200,
        "content inside the 72px gallery box should be visible"
    );
    assert_eq!(
        pixel(&pixmap, 50, 90),
        [255, 255, 255, 255],
        "gallery scroll-view must clip content below its 72px height"
    );
}
