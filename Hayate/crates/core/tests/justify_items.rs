use hayate_core::{
    AlignSelfValue, AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree,
    JustifyItemsValue, JustifySelfValue, StyleProp,
};

/// 100×100 の 1 セル grid に 40×40 のアイテムを 1 つ置き、コンテナ／アイテムへ
/// 整列プロパティを足してアイテムの原点を観測する。明示サイズ（40px）があるので
/// stretch では伸びず、start/end/center の差がそのまま原点 (x,y) に出る（issue #494）。
fn item_origin(container_extra: Vec<StyleProp>, item_extra: Vec<StyleProp>) -> (f32, f32) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);

    let mut root_style = vec![
        StyleProp::Display(DisplayValue::Grid),
        StyleProp::Width(Dimension::px(100.0)),
        StyleProp::Height(Dimension::px(100.0)),
        StyleProp::GridTemplateColumns(vec![Dimension::px(100.0)]),
        StyleProp::GridTemplateRows(vec![Dimension::px(100.0)]),
    ];
    root_style.extend(container_extra);
    tree.element_set_style(root, &root_style);

    let item = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, item);
    let mut item_style = vec![
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        StyleProp::Width(Dimension::px(40.0)),
        StyleProp::Height(Dimension::px(40.0)),
    ];
    item_style.extend(item_extra);
    tree.element_set_style(item, &item_style);

    tree.render(0.0);

    let rect = tree
        .element_layout_rect(item)
        .expect("grid item must have layout");
    (rect.0, rect.1)
}

#[test]
fn justify_items_start_keeps_item_at_the_inline_start() {
    // start: アイテムはセルの左端 → x = 0。
    let (x, _) = item_origin(vec![StyleProp::JustifyItems(JustifyItemsValue::Start)], vec![]);
    assert!(x.abs() < 1.0, "start aligns item to x=0, got {x}");
}

#[test]
fn justify_items_end_pushes_item_to_the_inline_end() {
    // end: アイテムはセルの右端 → x = 100 - 40 = 60。
    let (x, _) = item_origin(vec![StyleProp::JustifyItems(JustifyItemsValue::End)], vec![]);
    assert!((x - 60.0).abs() < 1.0, "end aligns item to x=60, got {x}");
}

#[test]
fn justify_items_center_centers_item_on_the_inline_axis() {
    // center: アイテムはセル中央 → x = (100 - 40) / 2 = 30。
    let (x, _) = item_origin(vec![StyleProp::JustifyItems(JustifyItemsValue::Center)], vec![]);
    assert!((x - 30.0).abs() < 1.0, "center aligns item to x=30, got {x}");
}

#[test]
fn justify_self_overrides_the_container_justify_items() {
    // コンテナは start だが、アイテムが justify-self: end で上書き → x = 60。
    let (x, _) = item_origin(
        vec![StyleProp::JustifyItems(JustifyItemsValue::Start)],
        vec![StyleProp::JustifySelf(JustifySelfValue::End)],
    );
    assert!(
        (x - 60.0).abs() < 1.0,
        "justify-self end overrides container start, x={x}"
    );
}

#[test]
fn justify_self_auto_follows_the_container_default() {
    // justify-self: auto はコンテナ既定（center）に従う → x = 30。
    let (x, _) = item_origin(
        vec![StyleProp::JustifyItems(JustifyItemsValue::Center)],
        vec![StyleProp::JustifySelf(JustifySelfValue::Auto)],
    );
    assert!(
        (x - 30.0).abs() < 1.0,
        "justify-self auto inherits container center, x={x}"
    );
}

// ── 既存 align-* が grid でも効くことを回帰で固定（issue #494） ──────────────

#[test]
fn align_items_still_aligns_grid_items_on_the_block_axis() {
    // align-items: center は grid のブロック軸（縦）でも効く → y = (100 - 40) / 2 = 30。
    let (_, y) = item_origin(vec![StyleProp::AlignItems(AlignValue::Center)], vec![]);
    assert!(
        (y - 30.0).abs() < 1.0,
        "align-items center centers grid item on block axis, y={y}"
    );
}

#[test]
fn align_self_overrides_align_items_in_grid() {
    // コンテナは flex-start（y=0）だが、アイテムが align-self: center で上書き → y = 30。
    let (_, y_default) = item_origin(vec![StyleProp::AlignItems(AlignValue::FlexStart)], vec![]);
    assert!(
        y_default.abs() < 1.0,
        "align-items flex-start keeps grid item at y=0, got {y_default}"
    );
    let (_, y) = item_origin(
        vec![StyleProp::AlignItems(AlignValue::FlexStart)],
        vec![StyleProp::AlignSelf(AlignSelfValue::Center)],
    );
    assert!(
        (y - 30.0).abs() < 1.0,
        "align-self center overrides container flex-start in grid, y={y}"
    );
}
