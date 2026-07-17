//! ビューポートバリアントの **レイアウト系プロップ**（`display` / `flex-direction` /
//! `width` 等）が、Canvas 経路でも実レイアウトへ適用されることの回帰テスト。
//!
//! 旧実装ではバリアントは `apply_visual` 経由でしか折り込まれず、`apply_visual` は
//! レイアウト系プロップを捨てる（`_ => {}`）うえ、レイアウト側（Taffy）は
//! `viewport_variants` を一切参照しなかった。結果、`maxWidth:719 → display:none` の
//! ような variant が **Canvas でだけ no-op** になり、狭幅でも優先度ラベルが消えず行が
//! 詰まらない乖離が出ていた（DOM はブラウザの `@media` で正しく隠れる）。
//!
//! ここでは retained な `scene_graph()` と ephemeral full-rebuild の双方を、リサイズの
//! 前後で検証する。

use hayate_core::{
    render_scene_graph, Color, Dimension, DisplayValue, DrawOp, ElementKind, ElementTree,
    FlexDirectionValue, RecordingPainter, StyleProp, ViewportCondition,
};

fn retained_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.into_ops()
}

fn fill_rects(ops: &[DrawOp]) -> Vec<([f32; 4], f32, f32, f32, f32)> {
    ops.iter()
        .filter_map(|op| match op {
            DrawOp::FillRect {
                x,
                y,
                width,
                height,
                color,
                ..
            } => Some((*color, *x, *y, *width, *height)),
            _ => None,
        })
        .collect()
}

fn ephemeral_fill_rects(tree: &ElementTree) -> Vec<([f32; 4], f32, f32, f32, f32)> {
    fill_rects(&tree.test_scene_full_rebuild_draw_ops())
}

fn text_run_count(ops: &[DrawOp]) -> usize {
    ops.iter()
        .filter(|op| matches!(op, DrawOp::DrawTextRun { .. }))
        .count()
}

fn max_width(w: f32) -> ViewportCondition {
    ViewportCondition {
        min_width: None,
        max_width: Some(w),
        min_height: None,
        max_height: None,
    }
}

/// `display:none` のレイアウト系 variant が、狭幅でサブツリー（box＋子 text）ごと消え、
/// 広幅に戻すと復活する。retained と ephemeral は常に一致する。
#[test]
fn display_none_variant_hides_subtree_across_resize() {
    let mut next = 1u64;
    let mut tree = ElementTree::new();
    tree.register_font(
        "Inter",
        include_bytes!("../assets/fonts/NotoSansJP.ttf").to_vec(),
    );

    let mut mk = |tree: &mut ElementTree, kind, styles: &[StyleProp]| {
        let id = tree.element_create(next, kind);
        next += 1;
        tree.element_set_style(id, styles);
        id
    };

    let root = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::BackgroundColor(Color::new(0.9, 0.9, 0.9, 1.0)),
            StyleProp::DefaultFontFamily("Inter".to_string()),
            StyleProp::DefaultColor(Color::BLACK),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    tree.set_root(root);
    tree.set_viewport(800.0, 600.0);

    // 常に見えるアンカー（青）。
    let anchor = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.element_append_child(root, anchor);

    // 優先度ラベル相当: maxWidth:719 で display:none になる View＋子 text（緑の箱）。
    let wrap = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(120.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style_variant(
        wrap,
        max_width(719.0),
        StyleProp::Display(DisplayValue::None),
    );
    let label = mk(&mut tree, ElementKind::Text, &[StyleProp::FontSize(11.0)]);
    tree.element_set_text(label, "優先度 中");
    tree.element_append_child(wrap, label);
    tree.element_append_child(root, wrap);

    const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
    let has_green = |rects: &[([f32; 4], f32, f32, f32, f32)]| {
        rects
            .iter()
            .any(|(c, _, _, w, h)| *c == GREEN && *w > 0.0 && *h > 0.0)
    };

    // ── 広幅 (800 > 719): variant 不成立 → ラベルは見える ──
    tree.render(0.0);
    let wide = retained_ops(&tree);
    assert!(has_green(&fill_rects(&wide)), "@800 緑の box が見える");
    assert_eq!(text_run_count(&wide), 1, "@800 ラベル text が描かれる");
    assert_eq!(
        fill_rects(&wide),
        ephemeral_fill_rects(&tree),
        "@800 retained==ephemeral"
    );

    // ── 狭幅 (390 <= 719): variant 成立 → サブツリーごと消える ──
    tree.set_viewport(390.0, 600.0);
    tree.render(16.0);
    let narrow = retained_ops(&tree);
    assert!(!has_green(&fill_rects(&narrow)), "@390 緑の box は消える");
    assert_eq!(
        text_run_count(&narrow),
        0,
        "@390 子 text のグリフも漏れない"
    );
    assert_eq!(
        fill_rects(&narrow),
        ephemeral_fill_rects(&tree),
        "@390 retained==ephemeral"
    );

    // ── 広幅へ戻す: ラベルが復活する ──
    tree.set_viewport(800.0, 600.0);
    tree.render(32.0);
    let back = retained_ops(&tree);
    assert!(
        has_green(&fill_rects(&back)),
        "@800 に戻すと緑の box が復活"
    );
    assert_eq!(text_run_count(&back), 1, "@800 に戻すとラベル text が復活");
    assert_eq!(
        fill_rects(&back),
        ephemeral_fill_rects(&tree),
        "戻し後も retained==ephemeral"
    );
}

/// `display` 以外のレイアウト系 variant（`width`）もリサイズで実レイアウトへ効く。
#[test]
fn dimension_variant_applies_to_layout_on_resize() {
    let mut next = 1u64;
    let mut tree = ElementTree::new();
    let mut mk = |tree: &mut ElementTree, styles: &[StyleProp]| {
        let id = tree.element_create(next, ElementKind::View);
        next += 1;
        tree.element_set_style(id, styles);
        id
    };

    let root = mk(
        &mut tree,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
        ],
    );
    tree.set_root(root);
    tree.set_viewport(800.0, 600.0);

    // 既定幅 100、maxWidth:719 で幅 40 に縮む箱。
    let box_id = mk(
        &mut tree,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style_variant(
        box_id,
        max_width(719.0),
        StyleProp::Width(Dimension::px(40.0)),
    );
    tree.element_append_child(root, box_id);

    let width_of = |tree: &ElementTree| -> f32 {
        fill_rects(&retained_ops(tree))
            .into_iter()
            .find(|(c, ..)| *c == [1.0, 0.0, 0.0, 1.0])
            .map(|(_, _, _, w, _)| w)
            .expect("red box")
    };

    tree.render(0.0);
    assert_eq!(width_of(&tree), 100.0, "@800 base width");

    tree.set_viewport(390.0, 600.0);
    tree.render(16.0);
    assert_eq!(
        width_of(&tree),
        40.0,
        "@390 variant width applies to layout"
    );
}
