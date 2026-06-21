//! アンカーシームの一致検証。保持型の差分走査と一時的な完全再構築（ADR-0079 の
//! golden-frame バックストップ）は同一の発行本体を共有し、アンカー戦略だけが
//! 異なる。よって任意の文書で両者の draw op 列は完全一致しなければならない。
//! 特に統合対象の box-shadow と text-input は、以前は各走査で重複していた。

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, FlexDirectionValue, Shadow, StyleProp,
    RecordingPainter, render_scene_graph,
};

/// draw op を Debug 文字列へ射影する（DrawOp は `PartialEq` でない）。`FillRect`
/// だけでなく shadow fill・box・text run・clip など全 op 種別を対象にする。
fn ops_debug(ops: Vec<DrawOp>) -> Vec<String> {
    ops.into_iter().map(|op| format!("{op:?}")).collect()
}

fn retained_ops(tree: &ElementTree) -> Vec<String> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    ops_debug(painter.into_ops())
}

fn ephemeral_ops(tree: &ElementTree) -> Vec<String> {
    ops_debug(tree.test_scene_full_rebuild_draw_ops())
}

fn assert_seam_parity(tree: &ElementTree, label: &str) {
    assert_eq!(
        retained_ops(tree),
        ephemeral_ops(tree),
        "retained scene diverged from ephemeral rebuild for {label}"
    );
}

const DROP: Shadow = Shadow {
    offset_x: 4.0,
    offset_y: 6.0,
    blur: 8.0,
    spread: 2.0,
    color: Color::new(0.0, 0.0, 0.0, 0.4),
    inset: false,
};
const INSET: Shadow = Shadow {
    offset_x: 0.0,
    offset_y: 2.0,
    blur: 5.0,
    spread: 0.0,
    color: Color::new(0.0, 0.0, 0.0, 0.3),
    inset: true,
};

#[test]
fn box_shadow_emission_matches_across_seams() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(120.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.5, 0.9, 1.0)),
            StyleProp::BorderRadius(10.0),
            StyleProp::BoxShadow(vec![DROP, INSET]),
        ],
    );
    tree.render(0.0);

    assert_seam_parity(&tree, "a box-shadowed (drop + inset) view");
}

#[test]
fn text_input_emission_matches_across_seams() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(1, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
        ],
    );
    tree.element_append_text_content(input, "hello world");
    tree.element_focus(input);
    tree.render(0.0);

    assert_seam_parity(&tree, "a focused text-input with content");
}

#[test]
fn shadow_and_text_input_nested_match_across_seams() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(280.0)),
            StyleProp::Height(Dimension::px(160.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::BackgroundColor(Color::new(0.95, 0.95, 0.95, 1.0)),
            StyleProp::BorderRadius(8.0),
            StyleProp::BoxShadow(vec![DROP]),
        ],
    );
    tree.element_append_child(root, input);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BoxShadow(vec![INSET]),
        ],
    );
    tree.element_append_text_content(input, "nested edit");
    tree.element_focus(input);
    tree.render(0.0);

    assert_seam_parity(&tree, "a shadowed view wrapping a shadowed text-input");
}
