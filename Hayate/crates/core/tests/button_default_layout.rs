//! `button` element-kind の UA デフォルトレイアウト（ADR-0109）。
//!
//! 明示的な `align-items` / `justify-content` を持たない `button` は、ブラウザの
//! `<button>` と同じレイアウトをする: cross 軸で垂直中央、main 軸で左寄せ
//! （flex-start）。これは button の base レイアウトスタイルに焼き込まれた core
//! 内のみの element-kind デフォルトで、アプリが指定する明示スタイルの下に位置する。

use hayate_core::{
    AlignValue, Dimension, ElementId, ElementKind, ElementTree, JustifyValue, StyleProp,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// テキストラベル 1 つを持つ button（高さ 36、水平 padding）を構築し、生成後に
/// `extra` スタイルを適用する。解決後の rect を読めるよう tree と button・label の
/// id を返す。
fn button_with_label(extra: &[StyleProp]) -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    tree.register_font("Inter", FONT.to_vec());
    let mut next = 1u64;
    let mut mk = |tree: &mut ElementTree, kind: ElementKind, styles: &[StyleProp]| {
        let id = tree.element_create(next, kind);
        next += 1;
        tree.element_set_style(id, styles);
        id
    };

    let root = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::DefaultFontFamily("Inter".to_string()),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);

    let mut button_styles = vec![
        StyleProp::Height(Dimension::px(36.0)),
        StyleProp::PaddingLeft(Dimension::px(14.0)),
        StyleProp::PaddingRight(Dimension::px(14.0)),
        StyleProp::DefaultFontSize(14.0),
    ];
    button_styles.extend_from_slice(extra);
    let button = mk(&mut tree, ElementKind::Button, &button_styles);

    let label = mk(&mut tree, ElementKind::Text, &[]);
    tree.element_set_text(label, "Click");
    tree.element_append_child(button, label);
    tree.element_append_child(root, button);

    let _ = tree.render(0.0);
    (tree, button, label)
}

fn rect(tree: &ElementTree, id: ElementId) -> (f32, f32, f32, f32) {
    tree.element_layout_rect(id)
        .expect("element must have a resolved rect")
}

#[test]
fn button_centers_label_on_cross_axis_by_default() {
    let (tree, button, label) = button_with_label(&[]);
    let (_bx, by, _bw, bh) = rect(&tree, button);
    let (_lx, ly, _lw, lh) = rect(&tree, label);

    let top_gap = ly - by;
    let bottom_gap = (by + bh) - (ly + lh);

    assert!(
        top_gap > 2.0,
        "label should sit below the button top, not clip to it (top gap = {top_gap})"
    );
    assert!(
        (top_gap - bottom_gap).abs() < 2.0,
        "label should be vertically centered: top gap {top_gap} ≈ bottom gap {bottom_gap}"
    );
}

#[test]
fn button_left_aligns_label_on_main_axis_by_default() {
    let (tree, button, label) = button_with_label(&[]);
    let (bx, _by, _bw, _bh) = rect(&tree, button);
    let (lx, _ly, _lw, _lh) = rect(&tree, label);

    // main 軸の flex-start: label は button の左コンテンツ端（左 padding = 14）に
    // 接し、中央寄せや右寄せにはならない。
    let left_gap = lx - bx;
    assert!(
        (left_gap - 14.0).abs() < 1.0,
        "label should be left-aligned at the left padding edge (left gap = {left_gap}, expected ≈ 14)"
    );
}

#[test]
fn explicit_align_items_overrides_button_default() {
    // アプリ指定の `align-items: flex-start` は kind デフォルト（`center`）に勝ち、
    // label を上端へ戻す（明示スタイル > kind デフォルト）。
    let (tree, button, label) = button_with_label(&[StyleProp::AlignItems(AlignValue::FlexStart)]);
    let (_bx, by, _bw, _bh) = rect(&tree, button);
    let (_lx, ly, _lw, _lh) = rect(&tree, label);

    let top_gap = ly - by;
    assert!(
        top_gap < 2.0,
        "explicit align-items: flex-start must override the centered default (top gap = {top_gap})"
    );
}

#[test]
fn explicit_justify_content_center_overrides_button_default() {
    // 水平中央を望む button は `justify-content: center` で明示的に opt-in する。
    // これは flex-start デフォルトに勝ち、content box 内で label の左右 gap を均す。
    let (tree, button, label) = button_with_label(&[
        StyleProp::Width(Dimension::px(200.0)),
        StyleProp::JustifyContent(JustifyValue::Center),
    ]);
    let (bx, _by, bw, _bh) = rect(&tree, button);
    let (lx, _ly, lw, _lh) = rect(&tree, label);

    let left_gap = lx - bx;
    let right_gap = (bx + bw) - (lx + lw);
    assert!(
        (left_gap - right_gap).abs() < 1.0,
        "explicit justify-content: center must center the label horizontally \
         (left gap {left_gap} ≈ right gap {right_gap})"
    );
    assert!(
        left_gap > 14.0,
        "centered label should sit past the left padding edge (left gap = {left_gap})"
    );
}

#[test]
fn non_button_kinds_keep_plain_taffy_default() {
    // UA の中央寄せは button 限定: 同形状の plain な `view` は Taffy デフォルト
    // （align-items: stretch）を保ち、子は全高に伸び上端に接する。中央寄せが他の
    // kind に漏れてはならない。
    let mut tree = ElementTree::new();
    tree.register_font("Inter", FONT.to_vec());
    let mut next = 1u64;
    let mut mk = |tree: &mut ElementTree, kind: ElementKind, styles: &[StyleProp]| {
        let id = tree.element_create(next, kind);
        next += 1;
        tree.element_set_style(id, styles);
        id
    };

    let view = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(36.0)),
            StyleProp::DefaultFontFamily("Inter".to_string()),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    tree.set_root(view);
    tree.set_viewport(300.0, 200.0);
    let label = mk(&mut tree, ElementKind::Text, &[]);
    tree.element_set_text(label, "Click");
    tree.element_append_child(view, label);
    let _ = tree.render(0.0);

    let (_vx, vy, _vw, _vh) = rect(&tree, view);
    let (_lx, ly, _lw, _lh) = rect(&tree, label);
    let top_gap = ly - vy;
    assert!(
        top_gap < 2.0,
        "a view must not inherit the button centering default (top gap = {top_gap})"
    );
}
