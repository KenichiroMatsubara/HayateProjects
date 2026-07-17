use hayate_core::{
    Color, Dimension, DisplayValue, ElementKind, ElementTree, GridAutoFlowValue,
    GridPlacementValue, StyleProp,
};

/// dense 詰めの差を観測するため、span-2 アイテムで穴を作る。3 列グリッドに span-2 を
/// 2 つ置くと 1 行目の最後の列が穴になる。疎配置（`row`）は後続の span-1 を穴の後ろへ
/// 流すが、dense（`row_dense`）はその穴を埋め直す。最後のアイテムの y が変わる（issue #493）。
fn third_item_origin_with_spans(flow: GridAutoFlowValue) -> (f32, f32) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(90.0, 60.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(90.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::GridTemplateColumns(vec![
                Dimension::px(30.0),
                Dimension::px(30.0),
                Dimension::px(30.0),
            ]),
            StyleProp::GridTemplateRows(vec![Dimension::px(30.0), Dimension::px(30.0)]),
            StyleProp::GridAutoFlow(flow),
        ],
    );
    // item0/item1 は 2 列ぶん占める → 1 行目に col2 の穴が残る。item2 は span-1。
    let mut items = Vec::new();
    for (i, id) in (11..=13).enumerate() {
        let item = tree.element_create(id, ElementKind::View);
        tree.element_append_child(root, item);
        let mut style = vec![StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))];
        if i < 2 {
            style.push(StyleProp::GridColumn(GridPlacementValue::span(2)));
        }
        tree.element_set_style(item, &style);
        items.push(item);
    }
    tree.render(0.0);

    let rect = tree
        .element_layout_rect(items[2])
        .expect("third grid item must have layout");
    (rect.0, rect.1)
}

#[test]
fn grid_auto_flow_sparse_leaves_the_hole_and_flows_past_it() {
    // row（疎）: item2 は穴を飛ばして 2 行目へ → (col2,row1) = (60, 30)。
    let (x, y) = third_item_origin_with_spans(GridAutoFlowValue::Row);
    assert!((x - 60.0).abs() < 1.0, "sparse item2 lands in col2, x={x}");
    assert!((y - 30.0).abs() < 1.0, "sparse item2 flows to row1, y={y}");
}

#[test]
fn grid_auto_flow_dense_backfills_the_hole() {
    // row_dense: item2 は 1 行目の穴を埋める → (col2,row0) = (60, 0)。
    let (x, y) = third_item_origin_with_spans(GridAutoFlowValue::RowDense);
    assert!((x - 60.0).abs() < 1.0, "dense item2 lands in col2, x={x}");
    assert!(
        (y - 0.0).abs() < 1.0,
        "dense item2 backfills the row0 hole, y={y}"
    );
}

/// 2×2 の明示グリッドに span-1 アイテムを 3 つ自動配置する。`row` は行を端から
/// 埋め（item1 は (col1,row0)）、`column` は列を端から埋める（item1 は (col0,row1)）。
/// auto-flow の主軸でアイテム 1 の位置が変わることを、解決後の幾何で固定する（issue #493）。
fn second_item_origin(flow: GridAutoFlowValue) -> (f32, f32) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Grid),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::GridTemplateColumns(vec![Dimension::px(50.0), Dimension::px(50.0)]),
            StyleProp::GridTemplateRows(vec![Dimension::px(50.0), Dimension::px(50.0)]),
            StyleProp::GridAutoFlow(flow),
        ],
    );
    let mut items = Vec::new();
    for id in 2..=4 {
        let item = tree.element_create(id, ElementKind::View);
        tree.element_append_child(root, item);
        tree.element_set_style(
            item,
            &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
        );
        items.push(item);
    }
    tree.render(0.0);

    let rect = tree
        .element_layout_rect(items[1])
        .expect("second grid item must have layout");
    (rect.0, rect.1)
}

#[test]
fn grid_auto_flow_row_fills_rows_first() {
    // row: item1 は最初の行の次の列 → (col1,row0) = (50, 0)。
    let (x, y) = second_item_origin(GridAutoFlowValue::Row);
    assert!(
        (x - 50.0).abs() < 1.0,
        "row flow places item1 at col1, x={x}"
    );
    assert!((y - 0.0).abs() < 1.0, "row flow keeps item1 in row0, y={y}");
}

#[test]
fn grid_auto_flow_column_fills_columns_first() {
    // column: item1 は最初の列の次の行 → (col0,row1) = (0, 50)。
    let (x, y) = second_item_origin(GridAutoFlowValue::Column);
    assert!(
        (x - 0.0).abs() < 1.0,
        "column flow keeps item1 in col0, x={x}"
    );
    assert!(
        (y - 50.0).abs() < 1.0,
        "column flow places item1 in row1, y={y}"
    );
}
