//! border-style の lowering: `none` は枠線を抑制、`dashed` は専用の dashed-border
//! draw op へ、`solid` は従来のエッジ描画を維持する。

use hayate_core::{
    render_scene_graph, BorderStyleValue, Color, Dimension, DrawOp, ElementKind, ElementTree,
    RecordingPainter, StyleProp,
};

fn draw_ops(tree: &mut ElementTree) -> Vec<DrawOp> {
    tree.render(0.0);
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.into_ops()
}

fn border_box(border_style: Option<BorderStyleValue>) -> ElementTree {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    let mut styles = vec![
        StyleProp::Width(Dimension::px(60.0)),
        StyleProp::Height(Dimension::px(60.0)),
        StyleProp::BorderWidth(6.0),
        StyleProp::BorderColor(Color::new(0.0, 0.0, 0.0, 1.0)),
    ];
    if let Some(bs) = border_style {
        styles.push(StyleProp::BorderStyle(bs));
    }
    tree.element_set_style(root, &styles);
    tree
}

#[test]
fn border_style_defaults_to_none_and_draws_no_border() {
    let mut tree = border_box(None);
    let ops = draw_ops(&mut tree);
    assert!(
        !ops.iter().any(|op| matches!(
            op,
            DrawOp::FillRoundedRing { .. } | DrawOp::DashedBorder { .. } | DrawOp::FillRect { .. }
        )),
        "default border-style none with border-width but no background must draw nothing, got {ops:?}"
    );
}

#[test]
fn border_style_solid_draws_border_edges() {
    let mut tree = border_box(Some(BorderStyleValue::Solid));
    let ops = draw_ops(&mut tree);
    let fills = ops
        .iter()
        .filter(|op| matches!(op, DrawOp::FillRect { .. }))
        .count();
    assert!(
        fills >= 4,
        "solid border with no background must draw the four edge rects, got {fills}"
    );
}

#[test]
fn border_style_dashed_emits_dashed_border_op() {
    let mut tree = border_box(Some(BorderStyleValue::Dashed));
    let ops = draw_ops(&mut tree);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::DashedBorder { .. })),
        "dashed border must lower to a DashedBorder draw op, got {ops:?}"
    );
}
