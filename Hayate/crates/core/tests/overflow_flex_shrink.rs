//! `overflow` プロップは視覚的クリップだけでなく Taffy にも届く。`visible` 以外の
//! overflow はボックスを CSS スクロールコンテナにし、その flex 自動最小サイズは 0 に
//! なるため、兄弟をはみ出さず残りスペースに縮む。`scroll_view_flex_shrink.rs` の
//! kind デフォルトに対する一般プロップ版で、ブラウザ(DOM モード)挙動と一致させる。

use hayate_core::{
    Dimension, ElementKind, ElementTree, FlexDirectionValue, OverflowValue, StyleProp,
};

const WINDOW_H: f32 = 800.0;
const BAR_H: f32 = 64.0;
const CONTENT_H: f32 = 2000.0;
const LEFTOVER: f32 = WINDOW_H - BAR_H; // 736: ボックスが縮むべき残りスペース

/// flex 縦列: 縮まない固定高バーの下に、子がウィンドウより高い `height: 100%` の
/// `view` を置く。ツリーと中段ボックスを返し、テストが `overflow` を切り替えて
/// 再計測できるようにする。
fn build(overflow: Option<OverflowValue>) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let bar = tree.element_create(2, ElementKind::View);
    let box_ = tree.element_create(3, ElementKind::View);
    let content = tree.element_create(4, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(1200.0, WINDOW_H);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        bar,
        &[
            StyleProp::Height(Dimension::px(BAR_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    let mut s = vec![
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::percent(100.0)),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
    ];
    if let Some(o) = overflow {
        s.push(StyleProp::Overflow(o));
    }
    tree.element_set_style(box_, &s);
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::px(CONTENT_H)),
            StyleProp::FlexShrink(0.0),
        ],
    );
    tree.element_append_child(root, bar);
    tree.element_append_child(root, box_);
    tree.element_append_child(box_, content);
    tree.render(0.0);
    (tree, box_)
}

fn box_height(tree: &ElementTree, box_: hayate_core::ElementId) -> f32 {
    tree.element_layout_rect(box_).unwrap().3
}

#[test]
fn overflow_hidden_view_shrinks_as_flex_item() {
    let (visible, vbox) = build(Some(OverflowValue::Visible));
    assert!(
        (box_height(&visible, vbox) - WINDOW_H).abs() < 0.5,
        "overflow:visible should not shrink (stays the full basis, overflowing): got {}",
        box_height(&visible, vbox)
    );

    let (hidden, hbox) = build(Some(OverflowValue::Hidden));
    assert!(
        (box_height(&hidden, hbox) - LEFTOVER).abs() < 0.5,
        "overflow:hidden is a scroll container and must shrink to {LEFTOVER}: got {}",
        box_height(&hidden, hbox)
    );
}

#[test]
fn toggling_overflow_at_runtime_re_runs_layout() {
    // visible 開始: ボックスは full basis を保ちウィンドウをはみ出す。
    let (mut tree, box_) = build(Some(OverflowValue::Visible));
    assert!((box_height(&tree, box_) - WINDOW_H).abs() < 0.5);

    // hidden へ切替: dual-routing が Taffy ノードを layout-dirty にし、次の
    // render で残りスペースに縮むこと。
    tree.element_set_style(box_, &[StyleProp::Overflow(OverflowValue::Hidden)]);
    tree.render(0.0);
    assert!(
        (box_height(&tree, box_) - LEFTOVER).abs() < 0.5,
        "toggling to overflow:hidden must re-run layout and shrink to {LEFTOVER}: got {}",
        box_height(&tree, box_)
    );

    // visible へ戻す: full basis まで再び広がる。
    tree.element_set_style(box_, &[StyleProp::Overflow(OverflowValue::Visible)]);
    tree.render(0.0);
    assert!(
        (box_height(&tree, box_) - WINDOW_H).abs() < 0.5,
        "toggling back to overflow:visible must re-run layout and restore {WINDOW_H}: got {}",
        box_height(&tree, box_)
    );
}
