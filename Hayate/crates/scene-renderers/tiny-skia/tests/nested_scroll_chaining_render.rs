//! CSS Gallery サンプル「nested scroll (chaining)」のピクセル回帰テスト。
//! 外側 `scroll-view` に入れ子の内側 `scroll-view` は、はみ出す内容を自分のボックスに
//! クリップしなければならない。これは静止時も外側スクロール後も同様で、内側 `Clip` が
//! 外側のスクロールオフセット `Group` 変換を正しく追従することを保証する。
//!
//! レイアウト（ビューポート空間、scale 1.0）、外側 scroll-view は原点：
//!
//! ```text
//!   outer scroll-view  (180 x 120)
//!   └─ column          (flex-direction: column)
//!      ├─ inner scroll-view (160 x 60)   ── screen y 0..60
//!      │  └─ green content  (160 x 200)  ── 内側ボックスにクリップ
//!      ├─ spacer            (160 x 20)   ── screen y 60..80  （透明な隙間）
//!      └─ blue tail         (160 x 100)  ── screen y 80..180 （外側でクリップ）
//! ```
//!
//! 内側の緑は高さ 200px だが 60px の内側ボックスに収まる。修正前のバグでは
//! 「Inner D」「Inner E」が内側ボックスを越えて「Outer tail」の行の上に描かれていた。
//! 透明な spacer は、兄弟の描画順に依存せず漏れを検出できる領域を与える。

use hayate_core::{
    Color, Dimension, ElementId, ElementKind, ElementTree, FlexDirectionValue, StyleProp,
};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn is_green(p: [u8; 4]) -> bool {
    p[1] > 200 && p[0] < 60 && p[2] < 60
}

fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 60 && p[1] < 60
}

/// nested-scroll-chaining のツリーを組み立て、`(tree, outer_scroll_id)` を返す。
fn nested_scroll_chaining_tree() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::ScrollView);
    let column = tree.element_create(2, ElementKind::View);
    let inner = tree.element_create(3, ElementKind::ScrollView);
    let green = tree.element_create(4, ElementKind::View);
    let spacer = tree.element_create(5, ElementKind::View);
    let tail = tree.element_create(6, ElementKind::View);

    tree.set_root(outer);
    tree.set_viewport(200.0, 200.0);

    tree.element_append_child(outer, column);
    tree.element_append_child(column, inner);
    tree.element_append_child(inner, green);
    tree.element_append_child(column, spacer);
    tree.element_append_child(column, tail);

    tree.element_set_style(
        outer,
        &[
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(180.0)),
            StyleProp::Height(Dimension::px(120.0)),
        ],
    );
    tree.element_set_style(
        column,
        &[
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(160.0)),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(60.0)),
        ],
    );
    tree.element_set_style(
        green,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(GREEN),
        ],
    );
    tree.element_set_style(
        spacer,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(20.0)),
        ],
    );
    tree.element_set_style(
        tail,
        &[
            StyleProp::Width(Dimension::px(160.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(BLUE),
        ],
    );

    tree.render(0.0);
    (tree, outer)
}

/// 静止時、内側 scroll-view のはみ出す内容は自分の 60px ボックスにクリップされ、
/// 透明な隙間や外側の tail に漏れない。
#[test]
fn inner_content_clipped_to_its_box_at_rest() {
    let (tree, _outer) = nested_scroll_chaining_tree();
    let mut pixmap = Pixmap::new(200, 200).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);

    // 60px の内側ボックス内で内容が見える。
    assert!(
        is_green(pixel(&pixmap, 80, 30)),
        "inner content should be visible inside its box, got {:?}",
        pixel(&pixmap, 80, 30)
    );
    // 内側ボックスの下の透明な隙間（y 60..80）。ここに内容が漏れてはならない。
    // 「Inner D/Inner E」の重なり領域。
    assert_eq!(
        pixel(&pixmap, 80, 70),
        [255, 255, 255, 255],
        "inner content must be clipped to its box, not bleed onto the gap"
    );
    // 隙間の下の外側 tail（y 80..120）は青い tail で、乱されていない。
    assert!(
        is_blue(pixel(&pixmap, 80, 100)),
        "outer tail should be visible and un-overlapped, got {:?}",
        pixel(&pixmap, 80, 100)
    );
}

/// 外側 scroll-view を 30px 上にスクロールしたとき、内側 `Clip` は未変換のローカル
/// 座標に留まらず、内側 scroll-view の新しい描画位置（外側スクロールオフセットの
/// `Group` 変換で上にずれる）を追従しなければならない。外側オフセット適用後：
///
/// ```text
///   inner box  ── screen y -30..30   （緑は下端 y=30 でクリップ）
///   gap        ── screen y  30..50   （透明）
///   blue tail  ── screen y  50..150  （外側で 50..120 にクリップ）
/// ```
///
/// 内側クリップがずれた場合（内容は上にずれて描かれるのにクリップは未変換のローカル
/// y 0..60 のまま）、緑が y=30..60 付近まで漏れて隙間を塗りつぶす。
#[test]
fn inner_clip_tracks_outer_scroll_offset_without_drift() {
    let (mut tree, outer) = nested_scroll_chaining_tree();
    tree.element_set_scroll_offset(outer, 0.0, 30.0);
    tree.render(0.0);

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);

    // （ずれた）内側ボックス上端付近で内容がまだ見える。
    assert!(
        is_green(pixel(&pixmap, 80, 15)),
        "inner content should remain visible after outer scroll, got {:?}",
        pixel(&pixmap, 80, 15)
    );
    // 隙間領域（screen y 30..50）。内側クリップは内容と一緒に上へ動くので、緑は
    // y=30 付近で止まらねばならない（隙間への漏れ＝ドリフトなし）。
    assert_eq!(
        pixel(&pixmap, 80, 45),
        [255, 255, 255, 255],
        "inner clip must track the outer scroll transform (no drift past y=30)"
    );
    // 外側 tail は screen y 50..120 に上へずれたが、まだ青い。
    assert!(
        is_blue(pixel(&pixmap, 80, 70)),
        "outer tail should be visible after outer scroll, got {:?}",
        pixel(&pixmap, 80, 70)
    );
}
