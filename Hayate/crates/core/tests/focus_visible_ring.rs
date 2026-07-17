//! `:focus-visible` パリティ(ADR-0102): Canvas Mode は Chromium のネイティブ
//! フォーカスリングを忠実に再現する。キーボード起点のフォーカスは任意要素にリングを
//! 出し、ポインタ起点はテキスト入力には出すがボタンには出さない。リングは core が
//! シーンに描くため、Canvas バックエンドはレンダラ個別の作業なしで描画できる。

use hayate_core::{
    render_scene_graph, Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, NodeKind,
    PseudoState, RecordingPainter, StyleProp,
};

/// シーン内の全フィル色を描画順で返す。
fn fill_colors(tree: &ElementTree) -> Vec<[f32; 4]> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter
        .into_ops()
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::FillRect { color, .. } => Some(color),
            _ => None,
        })
        .collect()
}

/// シーン内の全 `RoundedRing` ノード(x, y, width, height, border_width)。
fn rounded_rings(tree: &ElementTree) -> Vec<(f32, f32, f32, f32, f32)> {
    tree.scene_graph()
        .iter()
        .filter_map(|(_, node)| match node.kind {
            NodeKind::RoundedRing {
                x,
                y,
                width,
                height,
                border_width,
                ..
            } => Some((x, y, width, height, border_width)),
            _ => None,
        })
        .collect()
}

/// 原点に置いた 100×40 の `kind` 単一要素を、レイアウト・レンダリング済みで返す。
fn one_element(kind: ElementKind) -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let el = tree.element_create(2, kind);
    tree.element_set_style(
        el,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.element_append_child(root, el);
    tree.render(0.0);
    (tree, el)
}

#[test]
fn pointer_focused_button_is_not_focus_visible() {
    let (mut tree, button) = one_element(ElementKind::Button);
    tree.on_pointer_down_on(button, 10.0, 10.0);

    assert_eq!(
        tree.focus_visible_element(),
        None,
        "a button focused by pointer must not show the native focus ring"
    );
}

#[test]
fn focus_visible_emits_a_ring_outside_the_box() {
    // 要素ボックスは (0,0,100,40)。ネイティブフォーカスリングは外側から包む
    // (左上は厳密に負、ボックスより広い)。
    let (mut tree, input) = one_element(ElementKind::TextInput);
    tree.on_pointer_down_on(input, 10.0, 10.0);
    tree.render(16.0);

    let ring = rounded_rings(&tree)
        .into_iter()
        .find(|&(x, y, w, h, bw)| x < 0.0 && y < 0.0 && w > 100.0 && h > 40.0 && bw > 0.0);
    assert!(
        ring.is_some(),
        "expected a focus ring wrapping the box from outside, got rings: {:?}",
        rounded_rings(&tree)
    );
}

#[test]
fn focus_ring_is_not_clipped_by_the_elements_own_overflow() {
    // Chromium はフォーカスアウトラインを要素自身のクリップの外に描く。よって
    // `overflow: hidden` ではリングを要素のクリップノードの内側(ボックスに切り
    // 取られる)ではなく上位に付ける必要がある。
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::Overflow(hayate_core::OverflowValue::Hidden),
        ],
    );
    tree.element_append_child(root, input);
    tree.render(0.0);
    tree.on_pointer_down_on(input, 10.0, 10.0);
    tree.render(16.0);

    let sg = tree.scene_graph();
    let ring = sg
        .iter()
        .find(|(_, n)| matches!(n.kind, NodeKind::RoundedRing { .. }))
        .map(|(id, _)| id)
        .expect("focus ring present");
    let parent_is_clip = sg
        .parent_of(ring)
        .and_then(|p| sg.get(p))
        .is_some_and(|n| matches!(n.kind, NodeKind::Clip { .. }));
    assert!(
        !parent_is_clip,
        "focus ring must not be nested inside the element's own overflow clip"
    );
}

#[test]
fn app_focus_background_switch_still_works_alongside_the_ring() {
    const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
    const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);

    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(RED),
        ],
    );
    tree.element_set_pseudo_style(
        input,
        PseudoState::Focus,
        &[StyleProp::BackgroundColor(GREEN)],
    );
    tree.element_append_child(root, input);
    tree.render(0.0);

    let green = GREEN.to_array_f32();
    assert!(
        !fill_colors(&tree).contains(&green),
        "unfocused: background is the base colour"
    );

    tree.on_pointer_down_on(input, 10.0, 10.0);
    tree.render(16.0);

    assert!(
        fill_colors(&tree).contains(&green),
        "focused: the app's :focus background switch must still apply"
    );
    assert!(
        !rounded_rings(&tree).is_empty(),
        "focused: the native ring is drawn in addition to the :focus background"
    );
}

#[test]
fn pointer_focused_button_emits_no_ring() {
    // ボタンは自前のボーダーを持たないため、シーンにリングが一切ないこと。
    let (mut tree, button) = one_element(ElementKind::Button);
    tree.on_pointer_down_on(button, 10.0, 10.0);
    tree.render(16.0);

    assert!(
        rounded_rings(&tree).is_empty(),
        "a pointer-focused button must not draw a focus ring, got: {:?}",
        rounded_rings(&tree)
    );
}

#[test]
fn keyboard_focused_button_is_focus_visible() {
    let (mut tree, button) = one_element(ElementKind::Button);
    // フォーカス移動の前にキーボード操作(例: Tab)がある。
    tree.on_key_down("Tab", 0);
    tree.on_focus(button);

    assert_eq!(
        tree.focus_visible_element(),
        Some(button),
        "a button focused after a keyboard interaction shows the native focus ring"
    );
}

#[test]
fn pointer_focused_text_input_is_focus_visible() {
    let (mut tree, input) = one_element(ElementKind::TextInput);
    tree.on_pointer_down_on(input, 10.0, 10.0);

    assert_eq!(
        tree.focus_visible_element(),
        Some(input),
        "a text input always shows the native focus ring, even on pointer focus"
    );
}
