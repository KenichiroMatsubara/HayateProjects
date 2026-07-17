//! 編集選択ハイライトはフォーカス連動で、blur のライフサイクルは PointerKind 依存
//! （ADR-0104）。Mouse/Pen は範囲を覚え（Chromium のフォームコントロール準拠）、
//! Touch はキャレットに潰す（Android の挙動）。

use hayate_core::{
    render_scene_graph, Dimension, DrawOp, ElementId, ElementKind, ElementTree, FlexDirectionValue,
    PointerKind, RecordingPainter, StyleProp,
};

/// 編集選択ハイライトの色（Material の選択色、ADR-0097）。
const HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// 描画シーンに選択ハイライト矩形が含まれるか。
fn has_highlight(tree: &ElementTree) -> bool {
    !highlight_bands(tree).is_empty()
}

/// 各選択ハイライト矩形の縦帯 (y_min, y_max)。
fn highlight_bands(tree: &ElementTree) -> Vec<(f32, f32)> {
    draw_ops(tree)
        .iter()
        .filter_map(|op| match op {
            DrawOp::FillRect {
                y, height, color, ..
            } if *color == HIGHLIGHT_COLOR => Some((*y, *y + *height)),
            _ => None,
        })
        .collect()
}

/// 上にフォーカス済み text-input、下に blur 用にタップする空の `pad` view を持つ
/// 縦並びルート。(tree, input, pad) を返す。いずれもレイアウト済み。
fn input_with_outside(content: &str) -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let input = tree.element_create(2, ElementKind::TextInput);
    let pad = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_set_style(
        pad,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(120.0)),
        ],
    );
    tree.element_append_child(root, input);
    tree.element_append_child(root, pad);
    tree.element_append_text_content(input, content);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input, pad)
}

/// 上の入力内（内容は y≈20 の行）を Touch でドラッグ選択する。Touch なので選択の
/// chrome が表示される（ADR-0104）。chrome 系テストは blur/refocus による消去と復元を検証する。
fn drag_select(tree: &mut ElementTree) {
    tree.on_pointer_down_with_kind(2.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_move(70.0, 20.0);
}

#[test]
fn highlight_is_drawn_only_while_the_input_is_focused() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "a range is selected"
    );
    assert!(
        has_highlight(&tree),
        "the focused input paints its selection highlight",
    );

    // 範囲を潰さずにフィールドを blur する。範囲が EditState に保持されていても、
    // 非フォーカスの text-input はアクティブな選択ハイライトを描いてはならない
    // （ADR-0104、フォーカス連動ハイライト）。
    tree.element_blur(input);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "the range is still remembered after blur",
    );
    assert!(
        !has_highlight(&tree),
        "an unfocused text-input draws no selection highlight",
    );
}

#[test]
fn touch_blur_collapses_the_selection_and_dismisses_chrome() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "a range is selected"
    );
    assert!(
        tree.selection_toolbar().is_some(),
        "the selection shows chrome"
    );

    // Touch ポインタでフィールド外をタップ（Android の挙動）: 編集選択はキャレットに
    // 潰れ、選択 chrome は消える。
    tree.on_pointer_down_with_kind(100.0, 100.0, 0, PointerKind::Touch);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_none(),
        "a Touch blur collapses the edit selection to a caret",
    );
    assert!(
        tree.element_caret_byte_index(input).is_some(),
        "the caret survives the collapse",
    );
    assert!(
        tree.selection_toolbar().is_none(),
        "the selection chrome is dismissed after a Touch blur",
    );
}

#[test]
fn mouse_blur_remembers_the_range_and_refocus_restores_the_highlight() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    let range = tree
        .element_text_selection(input)
        .expect("a range is selected");

    // Mouse ポインタで外をタップするとフィールドは blur するが範囲は保持される
    // （Chromium のフォームコントロール準拠）。非フォーカス中はハイライトが隠れる。
    tree.on_pointer_down_with_kind(100.0, 100.0, 0, PointerKind::Mouse);
    tree.render(0.0);
    assert_eq!(
        tree.element_text_selection(input),
        Some(range),
        "a Mouse blur remembers the selected range",
    );
    assert!(!has_highlight(&tree), "the highlight hides while unfocused");

    // フィールドへフォーカスを戻す（例: Tab で戻る）と、覚えていた範囲が再点灯する。
    // フォーカス連動ハイライトが変わらず再表示される。
    tree.element_focus(input);
    tree.render(0.0);
    assert_eq!(
        tree.element_text_selection(input),
        Some(range),
        "the range is unchanged on refocus",
    );
    assert!(
        has_highlight(&tree),
        "refocusing restores the selection highlight"
    );
}

#[test]
fn unfocused_input_shows_no_chrome_even_when_it_remembers_a_range() {
    let (mut tree, input, _pad) = input_with_outside("hello world");

    drag_select(&mut tree);
    tree.render(0.0);
    assert!(
        tree.selection_toolbar().is_some(),
        "the focused selection shows chrome"
    );

    // 新たなポインタ操作なしにフォーカスを失う（例: フォーカスが他へ移る）と、Touch の
    // 範囲は残るが chrome は隠れる（active = focused、ADR-0104）。toolbar は非フォーカスの
    // フィールド上に残ってはならない。
    tree.element_blur(input);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "the range is still remembered",
    );
    assert!(
        tree.selection_toolbar().is_none(),
        "an unfocused text-input shows no selection chrome",
    );

    // Touch のままのフィールドを再フォーカスすると、覚えていた範囲とともに chrome が戻る。
    // chrome はフォーカス連動で、blur で消費されない。
    tree.element_focus(input);
    tree.render(0.0);
    assert!(
        tree.selection_toolbar().is_some(),
        "refocus restores the chrome"
    );
}

/// 200×240 ビューポート上、80px スペーサで隔てた高さ 40px の text-input 2 つの縦並び。
/// 上の行 y≈[0,40]、下の行 y≈[120,160]。スペーサは上の選択のフローティング toolbar を
/// 下の入力のヒット領域から離す（ADR-0097）。(tree, top, bottom) を返す。両者レイアウト済み、top がフォーカス。
fn two_inputs() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let top = tree.element_create(2, ElementKind::TextInput);
    let spacer = tree.element_create(3, ElementKind::View);
    let bottom = tree.element_create(4, ElementKind::TextInput);
    tree.set_root(root);
    tree.set_viewport(200.0, 240.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(240.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    for &inp in &[top, bottom] {
        tree.element_set_style(
            inp,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(40.0)),
                StyleProp::FontSize(16.0),
            ],
        );
    }
    tree.element_set_style(
        spacer,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(80.0)),
        ],
    );
    tree.element_append_child(root, top);
    tree.element_append_child(root, spacer);
    tree.element_append_child(root, bottom);
    tree.element_append_text_content(top, "hello world");
    tree.element_append_text_content(bottom, "hello world");
    tree.element_focus(top);
    tree.render(0.0);
    (tree, top, bottom)
}

#[test]
fn switching_text_inputs_never_lights_two_at_once() {
    let (mut tree, top, bottom) = two_inputs();

    // 上の入力（行は y≈[0,40]）で範囲を選択する。
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(70.0, 20.0);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(top).is_some(),
        "top has a range"
    );
    let bands = highlight_bands(&tree);
    assert!(!bands.is_empty(), "the top input is highlighted");
    assert!(
        bands.iter().all(|&(_, y1)| y1 <= 40.0),
        "only the top row lights up, got {bands:?}",
    );

    // 下の入力（行 y≈[120,160]）でドラッグ選択する。フォーカスがそちらへ移り、上の入力の
    // ハイライトが下のものと並んで残ってはならない（単一 active = focused、ADR-0104）。
    tree.on_pointer_down(2.0, 140.0);
    tree.on_pointer_move(70.0, 140.0);
    tree.render(0.0);
    assert!(
        tree.element_text_selection(bottom).is_some(),
        "bottom has a range"
    );
    let bands = highlight_bands(&tree);
    assert!(!bands.is_empty(), "the bottom input is highlighted");
    assert!(
        bands.iter().all(|&(y0, y1)| y0 > 40.0 && y1 > 40.0),
        "no highlight lingers over the (now unfocused) top input row [0,40], got {bands:?}",
    );
}
