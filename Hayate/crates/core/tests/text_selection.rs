//! 単一の selectable IFC 内（ADR-0097）と、1つの Selection Region 内の複数ブロックに
//! またがるドラッグ選択。

use hayate_core::{
    render_scene_graph, Dimension, DrawOp, ElementId, ElementKind, ElementTree, FlexDirectionValue,
    RecordingPainter, SelectionPoint, StyleProp, UserSelectValue,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// `<view [selectable]><text "Hello world"></view>` を1行で構築し
/// (tree, view, text) を返す。text 要素が IFC ルート。
fn selectable_paragraph(selectable: bool) -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, text);
    tree.element_set_text(text, "Hello world");
    if selectable {
        tree.element_set_selectable(view, true);
    }
    tree.render(0.0);
    (tree, view, text)
}

#[test]
fn drag_within_selectable_selects_anchor_to_focus_range() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // 行頭付近で押下し、複数グリフをまたいで右へドラッグ。
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    let sel = tree.selection().expect("a selection after dragging");
    let (start, end) = sel
        .range_within(text)
        .expect("both endpoints in the text element");
    assert!(
        start < end,
        "expected a non-empty range, got {start}..{end}"
    );
    assert_eq!(start, sel.anchor.offset.min(sel.focus.offset));
    assert!(
        sel.focus.offset > sel.anchor.offset,
        "focus should advance past the anchor when dragging rightwards",
    );
}

#[test]
fn pointer_down_in_selectable_collapses_to_a_caret() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    tree.on_pointer_down(40.0, 8.0);

    let sel = tree.selection().expect("a caret on press");
    assert!(sel.is_caret(), "press without drag is a collapsed caret");
    assert_eq!(sel.anchor.element, text);
}

#[test]
fn selected_range_lowers_a_highlight_rect_behind_the_text_run() {
    let (mut tree, _view, _text) = selectable_paragraph(true);

    // まだ選択なし: 背景のない段落の下にハイライト矩形は出ない。
    let before = draw_ops(&tree);
    let rects_before = before
        .iter()
        .filter(|op| matches!(op, DrawOp::FillRect { .. }))
        .count();

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);
    tree.render(0.0);

    let ops = draw_ops(&tree);
    let first_rect = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillRect { .. }));
    let first_text = ops
        .iter()
        .position(|op| matches!(op, DrawOp::DrawTextRun { .. }))
        .expect("the paragraph text run");

    let rect_idx = first_rect.expect("a highlight rect once a range is selected");
    assert!(
        ops.iter()
            .filter(|op| matches!(op, DrawOp::FillRect { .. }))
            .count()
            > rects_before,
        "selecting should add a highlight rect",
    );
    assert!(
        rect_idx < first_text,
        "highlight must paint behind (before) the text run",
    );
    if let DrawOp::FillRect { width, height, .. } = ops[rect_idx] {
        assert!(width > 0.0 && height > 0.0, "highlight has a visible area");
    }
}

/// `text` 内で現在選択中の部分文字列。ジェスチャ範囲のアサート用。
fn selected_text<'a>(tree: &ElementTree, text: ElementId, content: &'a str) -> &'a str {
    let sel = tree.selection().expect("a selection");
    let (start, end) = sel.range_within(text).expect("both endpoints in text");
    &content[start..end]
}

#[test]
fn double_click_selects_the_word_under_the_pointer() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // "Hello" 内の同じ位置を2回押すと単語全体に広がる。
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);

    assert_eq!(selected_text(&tree, text, "Hello world"), "Hello");
}

#[test]
fn triple_click_selects_the_whole_paragraph() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);

    assert_eq!(selected_text(&tree, text, "Hello world"), "Hello world");
}

const SHIFT: u32 = 1; // MODIFIER_SHIFT（proto/spec のワイヤ契約）。
const CTRL: u32 = 2; // MODIFIER_CTRL。

#[test]
fn select_all_covers_the_whole_region() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // 先にリージョン内へキャレットを置き（クリック）、その後 Ctrl+A。
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_key_down("a", CTRL);

    let sel = tree.selection().expect("a selection after Ctrl+A");
    let (start, end) = sel.range_within(text).expect("both endpoints in text");
    assert_eq!(
        (start, end),
        (0, "Hello world".len()),
        "whole region selected"
    );
}

#[test]
fn shift_arrow_extends_the_focus_by_one_character() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    tree.on_pointer_down(8.0, 8.0);
    let anchor = tree.selection().unwrap().anchor;
    let caret = tree.selection().unwrap().focus.offset;
    tree.on_pointer_up(8.0, 8.0);

    tree.on_key_down("ArrowRight", SHIFT);
    let sel = tree
        .selection()
        .expect("a selection after Shift+ArrowRight");
    assert_eq!(sel.anchor, anchor, "anchor stays fixed");
    assert!(
        sel.focus.offset > caret,
        "focus advances one character right"
    );

    // Shift+ArrowLeft はアンカーへ向けて（アンカー上まで）縮む。
    let extended = sel.focus.offset;
    tree.on_key_down("ArrowLeft", SHIFT);
    let sel = tree.selection().unwrap();
    assert!(
        sel.focus.offset < extended,
        "focus retreats, contracting the range"
    );
    let _ = text;
}

#[test]
fn shift_click_extends_focus_keeping_the_anchor_fixed() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // 行頭付近にキャレットを置き、行の先で Shift+クリック。
    tree.on_pointer_down(8.0, 8.0);
    let anchor = tree.selection().unwrap().anchor;
    tree.on_pointer_up(8.0, 8.0);

    tree.on_pointer_down_with(70.0, 8.0, SHIFT);

    let sel = tree.selection().expect("a selection after shift+click");
    assert_eq!(sel.anchor, anchor, "anchor must stay where the caret was");
    assert!(
        sel.focus.offset > sel.anchor.offset,
        "focus should extend past the anchor toward the shift+click",
    );
    let (start, end) = sel.range_within(text).expect("both endpoints in text");
    assert!(start < end, "shift+click should produce a non-empty range");
}

#[test]
fn selected_text_returns_the_dragged_substring() {
    let (mut tree, _view, _text) = selectable_paragraph(true);

    // 行頭から最初の単語をまたいでドラッグ。
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(40.0, 8.0);

    let copied = tree.selected_text().expect("text under the selection");
    let sel = tree.selection().unwrap();
    let (start, end) = sel.range_within(sel.anchor.element).unwrap();
    assert_eq!(copied, &"Hello world"[start..end]);
    assert!(!copied.is_empty(), "a non-empty drag yields some text");
}

#[test]
fn text_input_range_selection_lowers_a_highlight_behind_the_text() {
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
        ],
    );
    tree.element_append_text_content(input, "hello world");
    tree.element_focus(input);
    tree.render(0.0);

    // キャレットだけではハイライト帯は描かれない。
    assert!(
        highlight_bands(&tree).is_empty(),
        "a collapsed caret shows no selection highlight",
    );

    // 複数グリフをまたいでドラッグして描画すると、ハイライト帯が現れる。
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(70.0, 20.0);
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    assert!(
        !bands.is_empty(),
        "selecting a range inside the text-input lowers a highlight",
    );

    let ops = draw_ops(&tree);
    let first_highlight = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillRect { color, .. } if *color == HIGHLIGHT_COLOR))
        .expect("a highlight rect");
    let first_text = ops
        .iter()
        .position(|op| matches!(op, DrawOp::DrawTextRun { .. }))
        .expect("the field's text run");
    assert!(
        first_highlight < first_text,
        "the highlight must paint behind the text run",
    );
}

/// `<view [selectable]><text "Hello "><text "world" (bigger)></view>`: 異なるスタイルの
/// インライン子2つからなる1つの IFC。(tree, ifc root) を返す。
fn two_run_paragraph() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let lead = tree.element_create(2, ElementKind::Text);
    let tail = tree.element_create(3, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(lead, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, lead);
    tree.element_append_child(lead, tail);
    tree.element_set_text(lead, "Hello ");
    tree.element_set_text(tail, "world");
    tree.element_set_style(tail, &[StyleProp::FontSize(24.0)]);
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, lead)
}

#[test]
fn selected_text_joins_across_styled_inline_runs() {
    let (mut tree, _ifc) = two_run_paragraph();

    // 段落全体を選択。"Hello " / "world" のラン境界（1 IFC 内の異なるフォントサイズ2つ）を
    // またぐ。
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0); // トリプルクリックで段落を選択

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("Hello world"),
        "the copied text joins both styled runs in document order",
    );
}

/// 書き込みを記録する `Clipboard` 実装。実 OS クリップボードなしで、core が
/// Platform Adapter 境界へ押し出した内容をテストで検証できる。
#[derive(Default, Clone)]
struct RecordingClipboard {
    writes: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
}

impl hayate_core::Clipboard for RecordingClipboard {
    fn write_text(&self, text: &str) {
        self.writes.borrow_mut().push(text.to_string());
    }
}

#[test]
fn primary_c_writes_the_selection_through_the_clipboard_adapter() {
    let (mut tree, _view, _text) = selectable_paragraph(true);
    let clipboard = RecordingClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));

    // 範囲を選択して Ctrl/Cmd+C。
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(40.0, 8.0);
    let expected = tree.selected_text().expect("a non-empty selection");
    tree.on_pointer_up(40.0, 8.0);
    tree.on_key_down("c", CTRL);

    assert_eq!(
        clipboard.writes.borrow().as_slice(),
        &[expected],
        "the selected text is written once to the clipboard",
    );
}

#[test]
fn primary_c_without_a_selection_writes_nothing() {
    let (mut tree, _view, _text) = selectable_paragraph(true);
    let clipboard = RecordingClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));

    // キャレット（折りたたみ）は何も選択しないので、コピーは no-op。
    tree.on_pointer_down(40.0, 8.0);
    tree.on_pointer_up(40.0, 8.0);
    tree.on_key_down("c", CTRL);

    assert!(
        clipboard.writes.borrow().is_empty(),
        "copying an empty/caret selection must not write to the clipboard",
    );
}

#[test]
fn selected_text_is_none_for_a_collapsed_caret() {
    let (mut tree, _view, _text) = selectable_paragraph(true);

    // 単なる押下はキャレット（折りたたみ選択）を置くだけで、コピー対象はない。
    tree.on_pointer_down(40.0, 8.0);
    assert!(tree.selection().unwrap().is_caret());
    assert_eq!(tree.selected_text(), None);
}

#[test]
fn drag_over_user_select_none_does_not_start_a_selection() {
    // 選択は既定で境界フリー（ADR-0108）。`selectable` リージョンの不在はもはや
    // 選択を妨げず、オプトアウトは `user-select: none` による明示で、そのサブツリーを
    // 除外する。よってこの段落上のドラッグは何も開始しない。
    // （境界フリーの肯定ケース＝プレーンテキストのドラッグ選択は plain_text_selection.rs。）
    let (mut tree, _view, text) = selectable_paragraph(false);
    tree.element_set_user_select(text, UserSelectValue::None);
    tree.render(0.0);

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    assert!(
        tree.selection().is_none(),
        "user-select: none text must not start a selection",
    );
}

// --- 1つの Selection Region 内でのブロック横断選択 ---

/// 段落2つ（別々の IFC ブロック）を縦に積む `<view [selectable]>` を構築。
/// (tree, view, first, second) を返す。各段落は1行。
fn two_block_region(selectable: bool) -> (ElementTree, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let first = tree.element_create(2, ElementKind::Text);
    let second = tree.element_create(3, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(first, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(second, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, first);
    tree.element_append_child(view, second);
    tree.element_set_text(first, "First block");
    tree.element_set_text(second, "Second block");
    if selectable {
        tree.element_set_selectable(view, true);
    }
    tree.render(0.0);
    (tree, view, first, second)
}

/// 段落の行の垂直中心。クリック位置の算出用。
fn block_mid_y(tree: &ElementTree, block: ElementId) -> f32 {
    let (_, y, _, h) = tree.element_layout_rect(block).expect("a laid-out block");
    y + h / 2.0
}

#[test]
fn dragging_backwards_across_blocks_normalizes_to_document_order() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // 2番目のブロック内で押下し、1番目へ向けて上へドラッグ: アンカーは後方ブロック、
    // フォーカスは前方ブロックにある。
    tree.on_pointer_down(60.0, block_mid_y(&tree, second));
    tree.on_pointer_move(20.0, block_mid_y(&tree, first));

    let sel = tree.selection().expect("a cross-block selection");
    assert_eq!(
        sel.anchor.element, second,
        "anchor stays where the drag began"
    );
    assert_eq!(
        sel.focus.element, first,
        "focus follows the drag into block one"
    );

    let (start, end) = tree
        .selection_ordered()
        .expect("ordered endpoints for an active selection");
    assert_eq!(
        start.element, first,
        "document order puts the earlier block's point first",
    );
    assert_eq!(end.element, second, "the later block's point comes last");
}

#[test]
fn selected_text_joins_cross_block_selection_with_a_newline() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // 1番目の先頭から2番目の末尾まで選択し、範囲が両者間のブロックボックス（IFC ルート）
    // 境界をまたぐようにする。
    let applied = tree.set_selection_range(
        SelectionPoint::new(first, 0),
        SelectionPoint::new(second, "Second block".len()),
    );
    assert!(applied, "both blocks share one Selection Region");

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("First block\nSecond block"),
        "a cross-block copy joins blocks in document order with a single \\n at the block boundary",
    );
}

#[test]
fn cross_block_copy_follows_document_order_not_anchor_first() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // アンカーは後方ブロック、フォーカスは前方ブロック（後方へのドラッグ）。
    // コピー結果はアンカー優先ではなく、依然ドキュメント順で読めなければならない。
    let applied = tree.set_selection_range(
        SelectionPoint::new(second, "Second block".len()),
        SelectionPoint::new(first, 0),
    );
    assert!(applied, "both blocks share one Selection Region");

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("First block\nSecond block"),
        "copy joins blocks in document order regardless of drag direction",
    );
}

/// 段落3つ（3つの IFC ブロック）を縦に積む selectable な列を構築。
/// (tree, view, first, middle, last) を返す。各段落は1行。
fn three_block_region() -> (ElementTree, ElementId, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let first = tree.element_create(2, ElementKind::Text);
    let middle = tree.element_create(3, ElementKind::Text);
    let last = tree.element_create(4, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 300.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    for &block in &[first, middle, last] {
        tree.element_set_style(block, &[StyleProp::Width(Dimension::px(400.0))]);
        tree.element_append_child(view, block);
    }
    tree.element_set_text(first, "First block");
    tree.element_set_text(middle, "Middle block");
    tree.element_set_text(last, "Third block");
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, view, first, middle, last)
}

#[test]
fn user_select_none_block_is_excluded_from_the_copied_text() {
    let (mut tree, _view, first, middle, last) = three_block_region();
    // 中央の段落は選択をオプトアウト（CSS `user-select: none`）。
    tree.element_set_user_select(middle, UserSelectValue::None);
    tree.render(0.0);

    let applied = tree.set_selection_range(
        SelectionPoint::new(first, 0),
        SelectionPoint::new(last, "Third block".len()),
    );
    assert!(applied, "first and last share one Selection Region");

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("First block\nThird block"),
        "a user-select:none block contributes no text and leaves no orphan newline",
    );
}

// --- `user-select: contains` の包含境界（ADR-0108） ---

/// `selectable` な外側の列。`user-select: contains` ボックスで包まれた段落 `inside` の下に、
/// 外側 Selection Region を共有する素の段落 `outside` を置く。`contains` があると内側ボックスが
/// より狭い独自境界となり、なければ両段落は自由にまたがる。
/// (tree, contains-box, inside-paragraph, outside-paragraph) を返す。
fn contains_inside_region(boundary: bool) -> (ElementTree, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let boundary_box = tree.element_create(2, ElementKind::View);
    let inside = tree.element_create(3, ElementKind::Text);
    let outside = tree.element_create(4, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        boundary_box,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(inside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(outside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(outer, boundary_box);
    tree.element_append_child(boundary_box, inside);
    tree.element_append_child(outer, outside);
    tree.element_set_text(inside, "Inside box");
    tree.element_set_text(outside, "Outside box");
    tree.element_set_selectable(outer, true);
    if boundary {
        tree.element_set_user_select(boundary_box, UserSelectValue::Contains);
    }
    tree.render(0.0);
    (tree, boundary_box, inside, outside)
}

#[test]
fn contains_box_clamps_a_drag_inside_its_boundary() {
    let (mut tree, _box, inside, outside) = contains_inside_region(true);

    // `contains` ボックス内でドラッグを開始し、その外（ただし外側 selectable リージョン内）に
    // ある兄弟へ下へ引く。
    tree.on_pointer_down(20.0, block_mid_y(&tree, inside));
    tree.on_pointer_move(80.0, block_mid_y(&tree, outside));

    let sel = tree
        .selection()
        .expect("a selection started inside the contains box");
    assert_eq!(
        sel.focus.element, inside,
        "focus must stay clamped inside the `user-select: contains` boundary",
    );
}

/// `selectable` な外側の列。中央の子が段落2つ（`in_a`, `in_b`）を積む
/// `user-select: contains` ボックスで、同じ外側リージョン内でその後に段落 `outside` が続く。
/// (tree, in_a, in_b, outside) を返す。
fn contains_box_with_two_blocks() -> (ElementTree, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let boundary_box = tree.element_create(2, ElementKind::View);
    let in_a = tree.element_create(3, ElementKind::Text);
    let in_b = tree.element_create(4, ElementKind::Text);
    let outside = tree.element_create(5, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 300.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        boundary_box,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    for &block in &[in_a, in_b, outside] {
        tree.element_set_style(block, &[StyleProp::Width(Dimension::px(400.0))]);
    }
    tree.element_append_child(outer, boundary_box);
    tree.element_append_child(boundary_box, in_a);
    tree.element_append_child(boundary_box, in_b);
    tree.element_append_child(outer, outside);
    tree.element_set_text(in_a, "Alpha box");
    tree.element_set_text(in_b, "Beta box");
    tree.element_set_text(outside, "Gamma out");
    tree.element_set_selectable(outer, true);
    tree.element_set_user_select(boundary_box, UserSelectValue::Contains);
    tree.render(0.0);
    (tree, in_a, in_b, outside)
}

#[test]
fn contains_boundary_excludes_outside_blocks_from_copied_text() {
    let (mut tree, in_a, in_b, outside) = contains_box_with_two_blocks();

    // ボックス内の2段落にまたがる選択は許可される: コピー結果はドキュメント順で、
    // ブロック境界に `\n` を1つ入れて結合する。
    let inside = tree.set_selection_range(
        SelectionPoint::new(in_a, 0),
        SelectionPoint::new(in_b, "Beta box".len()),
    );
    assert!(
        inside,
        "both paragraphs lie inside the same `contains` boundary"
    );
    assert_eq!(
        tree.selected_text().as_deref(),
        Some("Alpha box\nBeta box"),
        "the two in-box paragraphs join; copy stays within the boundary",
    );

    // 境界を越えて外側の段落へまたがる範囲は即座に拒否され、外側テキストは決して連結されない。
    let leaked = tree.set_selection_range(
        SelectionPoint::new(in_a, 0),
        SelectionPoint::new(outside, "Gamma out".len()),
    );
    assert!(
        !leaked,
        "a range crossing the `contains` boundary is refused, never copied",
    );
}

#[test]
fn without_contains_a_drag_spans_freely_across_the_box() {
    // contains_box_clamps_a_drag_inside_its_boundary との対比。ボックスを素の view のまま
    // （`contains` なし）にすると、内部で開始したドラッグは兄弟段落へ流れ込む。既定は
    // 要素横断の自由なまたがりで、`contains` だけがそれをクランプする。
    let (mut tree, _box, inside, outside) = contains_inside_region(false);

    tree.on_pointer_down(20.0, block_mid_y(&tree, inside));
    tree.on_pointer_move(80.0, block_mid_y(&tree, outside));

    let sel = tree
        .selection()
        .expect("a selection started inside the box");
    assert_eq!(
        sel.focus.element, outside,
        "with no `contains` boundary the focus follows the drag into the sibling",
    );
}

/// ネストした `user-select: contains` ボックス2つ。`outer_block` を持つ外側境界の下に、
/// `inner_block` を持つ内側境界がある。どちらも包含境界だが別個のリージョン（最も近いものが勝つ）。
/// (tree, outer_block, inner_block) を返す。
fn nested_contains() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let outer_block = tree.element_create(2, ElementKind::Text);
    let inner = tree.element_create(3, ElementKind::View);
    let inner_block = tree.element_create(4, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(outer_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(inner_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(outer, outer_block);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, inner_block);
    tree.element_set_text(outer_block, "Outer box");
    tree.element_set_text(inner_block, "Inner box");
    tree.element_set_user_select(outer, UserSelectValue::Contains);
    tree.element_set_user_select(inner, UserSelectValue::Contains);
    tree.render(0.0);
    (tree, outer_block, inner_block)
}

#[test]
fn nested_contains_uses_the_innermost_boundary() {
    let (mut tree, outer_block, inner_block) = nested_contains();

    // 外側ボックスで開始したドラッグはネストボックスへ広がってはならない:
    // 内側 `contains` が `inner_block` のより近い境界。
    tree.on_pointer_down(20.0, block_mid_y(&tree, outer_block));
    tree.on_pointer_move(80.0, block_mid_y(&tree, inner_block));

    let sel = tree.selection().expect("a selection in the outer box");
    assert_eq!(
        sel.focus.element, outer_block,
        "focus stays in the outer box; the nested `contains` is its own boundary",
    );

    // 逆に、ネストボックス内で開始した新規ドラッグはそこにアンカーする。
    tree.on_pointer_down(20.0, block_mid_y(&tree, inner_block));
    let nested = tree.selection().expect("a caret in the nested box");
    assert_eq!(
        nested.anchor.element, inner_block,
        "a press in the nested box anchors to the innermost boundary",
    );
}

/// Material の選択ティント（ADR-0097）。draw op の中からハイライト矩形を識別する。
const HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

/// 各選択ハイライト矩形の垂直帯（y_min..y_max）。
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

#[test]
fn dragging_across_blocks_highlights_every_covered_block() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // 1番目の段落から2番目へ下へドラッグ: 選択は両ブロックにまたがるので、
    // 各ブロックが独自のハイライトを示さねばならない。
    tree.on_pointer_down(20.0, block_mid_y(&tree, first));
    tree.on_pointer_move(80.0, block_mid_y(&tree, second));
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    let (_, fy, _, fh) = tree.element_layout_rect(first).unwrap();
    let (_, sy, _, sh) = tree.element_layout_rect(second).unwrap();
    let covers = |y0: f32, y1: f32| bands.iter().any(|&(by0, by1)| by1 > y0 && by0 < y1);
    assert!(covers(fy, fy + fh), "the first block must be highlighted");
    assert!(covers(sy, sy + sh), "the second block must be highlighted");
}

#[test]
fn user_select_none_block_shows_no_highlight() {
    let (mut tree, _view, first, middle, last) = three_block_region();
    tree.element_set_user_select(middle, UserSelectValue::None);

    // 3ブロック全体を選択。中央は選択をオプトアウト。
    tree.set_selection_range(
        SelectionPoint::new(first, 0),
        SelectionPoint::new(last, "Third block".len()),
    );
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    // 各ブロックの垂直中心で帯を判定する。隣の帯がボックス端を1px かすめても
    // （行メトリクス vs ボックス幾何）、中央ブロックがハイライトされたと誤認しないため。
    let covered = |block: ElementId| {
        let mid = block_mid_y(&tree, block);
        bands.iter().any(|&(by0, by1)| by0 <= mid && mid <= by1)
    };
    assert!(covered(first), "the first block must be highlighted");
    assert!(covered(last), "the last block must be highlighted");
    assert!(
        !covered(middle),
        "a user-select:none block carries no highlight, just as it copies no text",
    );
}

#[test]
fn dragging_across_two_text_blocks_highlights_both_and_copies_them_joined() {
    // 要素横断の中核ケース（ADR-0108）。2つのテキストブロックにまたがる1ドラッグは、
    // 各ブロックにハイライトを描き、かつ両者をドキュメント順で結合してコピーする。
    // ハイライトとコピーは一致する。
    let (mut tree, _view, first, second) = two_block_region(true);

    // 1番目の段落の手前から2番目の末尾の先までドラッグ。
    tree.on_pointer_down(2.0, block_mid_y(&tree, first));
    tree.on_pointer_move(398.0, block_mid_y(&tree, second));
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    let covered = |block: ElementId| {
        let mid = block_mid_y(&tree, block);
        bands.iter().any(|&(by0, by1)| by0 <= mid && mid <= by1)
    };
    assert!(covered(first), "the first block is highlighted by the drag");
    assert!(
        covered(second),
        "the second block is highlighted by the drag"
    );

    // 同じドラッグは両ブロックをコピーし、ブロック境界に `\n` を1つだけ入れてドキュメント順で
    // 結合する。ドラッグ終端はピクセル依存のため、完全一致は上の API テストで固定し、ここでは
    // 構造だけを検証する: 1番目はブロック全体、2番目は先頭から。
    let copied = tree
        .selected_text()
        .expect("a cross-block drag copies text");
    assert_eq!(
        copied.matches('\n').count(),
        1,
        "one block-boundary newline: {copied:?}"
    );
    let (lead, tail) = copied.split_once('\n').unwrap();
    assert_eq!(lead, "First block", "the first block is copied whole");
    assert!(
        !tail.is_empty() && "Second block".starts_with(tail),
        "the second block is copied from its start, got {tail:?}",
    );
}

/// 非 selectable な列ルート。段落1つを持つ `selectable` view の `inner` の下に、
/// どの Selection Region にも属さない段落 `outside` を置く。
/// (tree, inner-paragraph, outside-paragraph) を返す。
fn region_with_outside_block() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let inner = tree.element_create(2, ElementKind::View);
    let inside = tree.element_create(3, ElementKind::Text);
    let outside = tree.element_create(4, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(inside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(outside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(root, inner);
    tree.element_append_child(inner, inside);
    tree.element_append_child(root, outside);
    tree.element_set_text(inside, "Inside region");
    tree.element_set_text(outside, "Outside region");
    tree.element_set_selectable(inner, true);
    tree.render(0.0);
    (tree, inside, outside)
}

#[test]
fn selection_does_not_leak_past_the_selectable_boundary() {
    let (mut tree, inside, outside) = region_with_outside_block();

    // リージョン内で開始し、その外にあるブロックへ下へドラッグ。
    tree.on_pointer_down(20.0, block_mid_y(&tree, inside));
    tree.on_pointer_move(80.0, block_mid_y(&tree, outside));
    tree.render(0.0);

    let sel = tree
        .selection()
        .expect("a selection started inside the region");
    assert_eq!(
        sel.focus.element, inside,
        "focus must stay clamped inside the Selection Region",
    );

    // 外側ブロックにハイライト帯は付かない。
    let (_, oy, _, oh) = tree.element_layout_rect(outside).unwrap();
    let leaked = highlight_bands(&tree)
        .iter()
        .any(|&(by0, by1)| by1 > oy && by0 < oy + oh);
    assert!(
        !leaked,
        "no highlight may appear outside the Selection Region"
    );
}

/// `outer_block` を持つ selectable な `outer` 列の下に、`inner_block` を持つネストした
/// selectable view を置く。両ブロックとも selectable だが、別々の Selection Region に属する
/// （最も近い祖先が勝つ）。(tree, outer_block, inner_block) を返す。
fn nested_regions() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let outer_block = tree.element_create(2, ElementKind::Text);
    let inner = tree.element_create(3, ElementKind::View);
    let inner_block = tree.element_create(4, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(outer_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(inner_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(outer, outer_block);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, inner_block);
    tree.element_set_text(outer_block, "Outer region");
    tree.element_set_text(inner_block, "Nested region");
    tree.element_set_selectable(outer, true);
    tree.element_set_selectable(inner, true);
    tree.render(0.0);
    (tree, outer_block, inner_block)
}

#[test]
fn nested_region_uses_the_nearest_selectable_ancestor() {
    let (mut tree, outer_block, inner_block) = nested_regions();

    // 外側リージョンで開始したドラッグはネストリージョンへ広がってはならない:
    // ネストした `selectable` が `inner_block` のより近い祖先。
    tree.on_pointer_down(20.0, block_mid_y(&tree, outer_block));
    tree.on_pointer_move(80.0, block_mid_y(&tree, inner_block));

    let sel = tree.selection().expect("a selection in the outer region");
    assert_eq!(
        sel.focus.element, outer_block,
        "focus stays in the outer region; the nested region is its own boundary",
    );

    // 逆に、ネストリージョン内で開始した新規ドラッグはそこで選択する。
    tree.on_pointer_down(20.0, block_mid_y(&tree, inner_block));
    let nested = tree.selection().expect("a caret in the nested region");
    assert_eq!(
        nested.anchor.element, inner_block,
        "a press in the nested region anchors to the nested block",
    );
}
