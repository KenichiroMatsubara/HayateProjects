//! `grid-column` / `grid-row`（新 wire 型 grid-placement、issue #495）の明示配置を
//! 実 `ElementTree` 駆動で固定する。3 列 × 2 行（各 30px）の明示グリッドに 1 つの
//! アイテムを置き、解決後の幾何 `(x, y, w, h)` で配置・占有を assert する。

use hayate_core::{
    Color, Dimension, DisplayValue, ElementKind, ElementTree, GridLineValue, GridPlacementValue,
    StyleProp,
};

const TRACK: f32 = 30.0;

/// 3 列 × 2 行（各 30px）の明示グリッドに 1 アイテムを置き、その layout rect を返す。
fn placed_item_rect(placement: Vec<StyleProp>) -> (f32, f32, f32, f32) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(90.0, 60.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(3.0 * TRACK)),
            StyleProp::Height(Dimension::px(2.0 * TRACK)),
            StyleProp::GridTemplateColumns(vec![
                Dimension::px(TRACK),
                Dimension::px(TRACK),
                Dimension::px(TRACK),
            ]),
            StyleProp::GridTemplateRows(vec![Dimension::px(TRACK), Dimension::px(TRACK)]),
        ],
    );

    let item = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, item);
    let mut style = vec![StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))];
    style.extend(placement);
    tree.element_set_style(item, &style);
    tree.render(0.0);

    tree.element_layout_rect(item)
        .expect("item must have layout")
}

#[test]
fn grid_column_line_places_item_in_that_column() {
    // grid-column: 2 → 2 列目（グリッド線 2 = 1 列目と 2 列目の境界）に乗る。
    let (x, _y, w, _h) = placed_item_rect(vec![StyleProp::GridColumn(GridPlacementValue::line(2))]);
    assert!(
        (x - TRACK).abs() < 1.0,
        "grid-column: 2 lands in column 2, x={x}"
    );
    assert!(
        (w - TRACK).abs() < 1.0,
        "single-line placement spans one track, w={w}"
    );
}

#[test]
fn grid_row_line_places_item_in_that_row() {
    // grid-row: 2 → 2 行目（グリッド線 2）に乗る。
    let (_x, y, _w, h) = placed_item_rect(vec![StyleProp::GridRow(GridPlacementValue::line(2))]);
    assert!((y - TRACK).abs() < 1.0, "grid-row: 2 lands in row 2, y={y}");
    assert!(
        (h - TRACK).abs() < 1.0,
        "single-line placement spans one track, h={h}"
    );
}

#[test]
fn grid_column_span_occupies_multiple_tracks() {
    // grid-column: span 2 → 先頭から 2 列ぶん占有（幅 = 2 トラック）。
    let (x, _y, w, _h) = placed_item_rect(vec![StyleProp::GridColumn(GridPlacementValue::span(2))]);
    assert!(
        (x - 0.0).abs() < 1.0,
        "auto-placed span starts at column 1, x={x}"
    );
    assert!(
        (w - 2.0 * TRACK).abs() < 1.0,
        "span 2 occupies two tracks, w={w}"
    );
}

#[test]
fn grid_column_start_end_spans_the_explicit_range() {
    // grid-column: 2 / 4 → 2 列目から 4 番目のグリッド線まで（2 列ぶん）。
    let (x, _y, w, _h) = placed_item_rect(vec![StyleProp::GridColumn(GridPlacementValue::new(
        GridLineValue::Line(2),
        GridLineValue::Line(4),
    ))]);
    assert!(
        (x - TRACK).abs() < 1.0,
        "start line 2 begins at column 2, x={x}"
    );
    assert!(
        (w - 2.0 * TRACK).abs() < 1.0,
        "lines 2..4 cover two tracks, w={w}"
    );
}
