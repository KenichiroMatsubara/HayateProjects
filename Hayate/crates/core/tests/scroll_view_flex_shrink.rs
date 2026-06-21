//! scroll-view は CSS スクロールコンテナなので、flex item としての自動最小
//! サイズは 0 になり、兄弟が残した空間まで縮む。固定高の兄弟の分だけ親から
//! はみ出すことはない。
//!
//! リグレッション: flex column 内で固定高 AppBar の下に `height: 100%` の
//! scroll-view が並ぶ構成（Tasks / CSS Gallery 両ページ）で、修正前の Canvas
//! モードは scroll-view をウィンドウ全高でレイアウトし、AppBar の高さ分だけ
//! 下にはみ出していた。その膨らんだボックス高はスクロールビューポート
//! （`element_scroll_max_offset`）でもあるため、末尾の AppBar 高さ分の内容に
//! 到達できなかった（DOM モードのネイティブスクロールでは到達できた）。
//! ScrollView を Taffy のスクロールコンテナとして印付けることでブラウザの
//! shrink-to-fit を回復する。Tasks 版（`flex-grow: 1`）と Gallery 版
//! （`height: 100%` のみ）の両方をカバーする。

use hayate_core::{Dimension, ElementKind, ElementTree, FlexDirectionValue, StyleProp};

const WINDOW_H: f32 = 800.0;
const APPBAR_H: f32 = 64.0;
const CONTENT_H: f32 = 2000.0;
const PAD_BOTTOM: f32 = 28.0;

fn build(tasks_variant: bool) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let appbar = tree.element_create(2, ElementKind::View);
    let scroll = tree.element_create(3, ElementKind::ScrollView);
    let content = tree.element_create(4, ElementKind::View);

    tree.set_root(root);
    tree.set_viewport(1200.0, WINDOW_H);

    // root: ウィンドウ全体の flex column
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    // appbar: 固定 64px 高。実アプリでは内容（ロゴ・ボタン）で 64px を保つので、
    // 潰れないよう flex-shrink:0 でモデル化する。
    tree.element_set_style(
        appbar,
        &[
            StyleProp::Height(Dimension::px(APPBAR_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    // scroll-view: App.tsx / CssGallery.tsx のパターン。
    let mut sv_style = vec![
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::percent(100.0)),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::PaddingBottom(Dimension::px(PAD_BOTTOM)),
    ];
    if tasks_variant {
        sv_style.push(StyleProp::FlexGrow(1.0));
    }
    tree.element_set_style(scroll, &sv_style);
    // 背の高い content 子。実アプリではこの列の高さは多数の子から決まり、
    // min-content（min-height:auto）が内容より縮むのを防ぐ。flex-shrink:0 で
    // モデル化する。
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::px(CONTENT_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );

    tree.element_append_child(root, appbar);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

fn check(tasks_variant: bool) {
    let label = if tasks_variant { "Tasks" } else { "Gallery" };
    let (tree, scroll) = build(tasks_variant);

    let (_, sv_top, _, view_h) = tree.element_layout_rect(scroll).unwrap();
    let (_max_x, max_y) = tree.element_scroll_max_offset(scroll);

    // scroll-view は AppBar の下から始まり、残りの高さだけを満たす。
    let expected_viewport = WINDOW_H - APPBAR_H;
    // スクロール可能な内容 = content 子 + scroll-view の下パディング。
    let expected_max = (CONTENT_H + PAD_BOTTOM) - expected_viewport;

    assert!(
        (sv_top - APPBAR_H).abs() < 0.5,
        "{label}: scroll-view top {sv_top}, expected {APPBAR_H} (below the AppBar)"
    );
    assert!(
        (view_h - expected_viewport).abs() < 0.5,
        "{label}: scroll-view viewport {view_h}, expected {expected_viewport} \
         (window {WINDOW_H} - appbar {APPBAR_H}); it must shrink, not overflow"
    );
    assert!(
        (max_y - expected_max).abs() < 0.5,
        "{label}: max scroll {max_y}, expected {expected_max} — short by {} (content unreachable)",
        expected_max - max_y
    );
}

#[test]
fn tasks_page_scroll_view_shrinks_and_reaches_bottom() {
    check(true);
}

#[test]
fn gallery_page_scroll_view_shrinks_and_reaches_bottom() {
    check(false);
}
