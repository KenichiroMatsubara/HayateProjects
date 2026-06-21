//! Retained 差分 lowering は、レイアウト reflow 後に古い box ジオメトリを描いて
//! はならない。要素の追加・削除・選択は flex reflow を祖先（伸びる panel）や
//! 兄弟（押し下げられる）へ波及させるが、それらの box は自身が
//! structure/visual-dirty にはならない。
//!
//! 差分ハーネスを正とする: 各変更後、retained な `scene_graph()` の fill rect は
//! ephemeral な full-rebuild の fill rect と一致しなければならない。乖離があれば
//! retained 側のジオメトリが古い。（テストシーンは塗りつぶしの `View` box のみで、
//! fill rect が全描画要素を捉える。）

use hayate_core::{
    Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, FlexDirectionValue,
    RecordingPainter, StyleProp, render_scene_graph,
};

/// 各 FillRect の (color, x, y, w, h) を描画順で返す。
type Rects = Vec<([f32; 4], f32, f32, f32, f32)>;

fn project(ops: Vec<DrawOp>) -> Rects {
    ops.into_iter()
        .filter_map(|op| match op {
            DrawOp::FillRect {
                x,
                y,
                width,
                height,
                color,
                ..
            } => Some((color, x, y, width, height)),
            _ => None,
        })
        .collect()
}

fn retained_rects(tree: &ElementTree) -> Rects {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    project(painter.into_ops())
}

fn ephemeral_rects(tree: &ElementTree) -> Rects {
    project(tree.test_scene_full_rebuild_draw_ops())
}

/// retained シーンが full ephemeral rebuild と fill 単位で一致することを検証する。
fn assert_parity(tree: &ElementTree, label: &str) {
    assert_eq!(
        retained_rects(tree),
        ephemeral_rects(tree),
        "retained scene diverged from ephemeral rebuild after {label}"
    );
}

fn footer_y(tree: &ElementTree) -> f32 {
    for (color, _x, y, _w, _h) in retained_rects(tree) {
        if color == [1.0, 0.0, 0.0, 1.0] {
            return y;
        }
    }
    panic!("footer rect not found");
}

fn card(tree: &mut ElementTree, next_id: &mut u64, color: Color) -> ElementId {
    let id = tree.element_create(*next_id, ElementKind::View);
    *next_id += 1;
    tree.element_set_style(
        id,
        &[
            StyleProp::Height(Dimension::px(20.0)),
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::BackgroundColor(color),
        ],
    );
    id
}

/// root(col) > [ board(col){ card } , footer ]
/// card 追加で `board` が伸び `footer` が押し下がる。`footer` は insert で dirty
/// 化されないため、その retained box が staleness のカナリアになる。
fn studio() -> (ElementTree, u64, ElementId, ElementId, ElementId) {
    let mut next_id = 1u64;
    let mut tree = ElementTree::new();
    let root = tree.element_create(next_id, ElementKind::View);
    next_id += 1;
    tree.set_root(root);
    tree.set_viewport(200.0, 400.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::BackgroundColor(Color::new(0.9, 0.9, 0.9, 1.0)),
        ],
    );

    let board = tree.element_create(next_id, ElementKind::View);
    next_id += 1;
    tree.element_set_style(
        board,
        &[StyleProp::FlexDirection(FlexDirectionValue::Column)],
    );
    tree.element_append_child(root, board);

    let footer = tree.element_create(next_id, ElementKind::View);
    next_id += 1;
    tree.element_set_style(
        footer,
        &[
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, footer);

    let first = card(&mut tree, &mut next_id, Color::new(0.0, 0.0, 1.0, 1.0));
    tree.element_append_child(board, first);

    (tree, next_id, root, board, footer)
}

#[test]
fn footer_follows_board_growth_on_incremental_insert() {
    let (mut tree, mut next_id, _root, board, _footer) = studio();
    tree.render(0.0);
    assert_parity(&tree, "initial render");
    let y0 = footer_y(&tree);
    assert!((y0 - 20.0).abs() < 0.01, "footer starts below one card, got {y0}");

    // 差分 insert: board が card 1 つ分伸び、footer は下へずれなければならない。
    let c2 = card(&mut tree, &mut next_id, Color::new(0.0, 0.0, 1.0, 1.0));
    tree.element_append_child(board, c2);
    tree.render(16.0);

    assert_parity(&tree, "add second card");
    let y1 = footer_y(&tree);
    assert!(
        (y1 - 40.0).abs() < 0.01,
        "footer should be pushed to y=40 after a second card, got {y1}"
    );
}

#[test]
fn parity_holds_across_repeated_inserts_and_removes() {
    let (mut tree, mut next_id, _root, board, _footer) = studio();
    tree.render(0.0);
    assert_parity(&tree, "initial render");

    let mut cards = Vec::new();
    for _ in 0..4 {
        let c = card(&mut tree, &mut next_id, Color::new(0.0, 0.0, 1.0, 1.0));
        tree.element_append_child(board, c);
        cards.push(c);
        tree.render(16.0);
        assert_parity(&tree, "after insert");
    }

    for c in cards {
        tree.element_remove(c);
        tree.render(16.0);
        assert_parity(&tree, "after remove");
    }
}

#[test]
fn insert_before_shifts_following_sibling() {
    let (mut tree, mut next_id, _root, board, _footer) = studio();
    tree.render(0.0);
    let first_child = tree.ordered_children(board)[0];

    // 既存の先頭 card の前に card を挿入: 既存 card が下へずれ footer も追従する。
    // どちらも insert 自体の dirty マークでは触れられない。
    let inserted = card(&mut tree, &mut next_id, Color::new(0.0, 1.0, 0.0, 1.0));
    tree.element_insert_before(board, inserted, first_child);
    tree.render(16.0);

    assert_parity(&tree, "insert before first card");
    let y = footer_y(&tree);
    assert!((y - 40.0).abs() < 0.01, "footer pushed to y=40, got {y}");
}
