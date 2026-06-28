//! ElementTree の公開入力ハンドラ経由で EditState を検証する（ADR-0069）。

use hayate_core::{
    Clipboard, CompositionClause, CompositionUnderline, Dimension, Direction, EditIntent,
    ElementKind, ElementTree, Granularity, PointerKind, StyleProp,
};
use std::cell::RefCell;
use std::rc::Rc;

const SHIFT: u32 = 1; // MODIFIER_SHIFT（proto/spec のワイヤ契約）。
const ALT: u32 = 4; // MODIFIER_ALT — macOS の Option（単語単位）修飾キー。

/// `content` を保持しキャレットが末尾にあるフォーカス済みテキスト入力。
/// キャレット/選択インデックスのアサーションにはレイアウト不要。
fn focused_input(content: &str) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let input = tree.element_create(100, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, content);
    (tree, input)
}

#[test]
fn bare_arrow_moves_the_caret_one_grapheme() {
    // 素の矢印キーでキャレットを移動する（ADR-0103）。"aあb" でマルチバイト
    // grapheme 単位の移動を確認する。
    let (mut tree, input) = focused_input("aあb"); // キャレットは末尾 (5)

    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(tree.element_caret_byte_index(input), Some(4), "retreats past 'b'");
    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(tree.element_caret_byte_index(input), Some(1), "retreats past 'あ'");

    tree.on_key_down("ArrowRight", 0);
    assert_eq!(tree.element_caret_byte_index(input), Some(4), "advances past 'あ'");

    // 素の矢印は選択を折りたたんだまま（範囲ではなくキャレット）にする。
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn bare_arrow_over_a_selection_collapses_to_its_edge() {
    // 範囲選択中の素の矢印は、端を越えて移動せず端へ折りたたむ
    // （Chromium <input> の挙動）。
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾 (5)
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT); // "lo" を選択 → 範囲 (3,5)
    assert_eq!(tree.element_text_selection(input), Some((3, 5)));

    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(3),
        "collapses to the left edge of the former selection",
    );
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn apply_edit_intent_is_the_os_independent_entry_point() {
    // Platform Adapter は OS のキー入力を intent に変換しこの seam を直接駆動する。
    // core はどのキーが起点かを知らない（ADR-0103）。
    let (mut tree, input) = focused_input("hello"); // キャレットは 5

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(4));

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_text_selection(input), Some((3, 4)));
}

#[test]
fn boundary_intents_move_and_extend_the_caret_to_the_field_ends() {
    // Home/End と Ctrl+Home/End はアダプタで Line/Doc 境界 intent に変換され、
    // core は OS 非依存の seam を通して適用する（ADR-0103）。単一行では
    // すべての境界がフィールド端となる。
    let (mut tree, input) = focused_input("hello world"); // キャレットは末尾 (11)

    // Home (Move/LineBoundary/Backward) はキャレットを先頭へ折りたたむ。
    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(0), "Home → field start");
    assert!(tree.element_text_selection(input).is_none(), "a Move stays collapsed");

    // Shift+End (Extend/LineBoundary/Forward) はキャレットから末尾まで選択する。
    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Extend {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        },
    ));
    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 11)),
        "Shift+End extends the selection to the field end, anchor fixed at 0",
    );

    // Ctrl+Home (Move/DocBoundary/Backward) は先頭へ折りたたむ。
    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::DocBoundary,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(0));
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn arrow_keys_do_not_disturb_an_active_ime_composition() {
    // IME preedit がアクティブな間のキャレットキーは、編集も composition の破壊も
    // してはならない（ADR-0103）。intent は消費されず、preedit と内容は保持される。
    let mut tree = ElementTree::new();
    let input = tree.element_create(101, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "ab"); // キャレットは 2
    tree.on_composition_start(input, "きゅ"); // アクティブな preedit

    let consumed = tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Backward,
        },
    );
    assert!(!consumed, "composition 中は intent を拒否する");
    assert_eq!(tree.element_caret_byte_index(input), Some(2), "キャレットは不動");
    assert_eq!(
        tree.element_get_text_content(input),
        "abきゅ",
        "composition はそのまま保持される",
    );
}

#[test]
fn delete_keys_do_not_disturb_an_active_ime_composition() {
    // IME preedit がアクティブな間の Backspace/Delete は、確定済み内容の編集も
    // composition の破壊もしてはならない（ADR-0103）。composition 中は seam で
    // intent を拒否するため、preedit と内容は保持される。
    let mut tree = ElementTree::new();
    let input = tree.element_create(102, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "ab"); // キャレットは 2
    tree.on_composition_start(input, "きゅ"); // アクティブな preedit

    tree.on_key_down("Backspace", 0);
    tree.on_key_down("Delete", 0);

    assert_eq!(
        tree.element_get_text_content(input),
        "abきゅ",
        "どちらのキーも確定テキストや composition を変えない",
    );
}

#[test]
fn shift_arrow_extends_text_input_selection_then_typing_replaces_it() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(10, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "hello"); // キャレットは末尾

    // Shift+ArrowLeft 2回で末尾2文字 ("lo") を選択する。
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT);

    // 範囲に上書き入力すると置換される（replace-on-type）。
    tree.on_text_input(input, "X");
    assert_eq!(tree.element_get_text_content(input), "helX");
}

/// `content` を保持しレイアウト済みのフォーカス済みテキスト入力。ポインタ/キー
/// ジェスチャを受けられる。(tree, input) を返す。
fn text_input_with(content: &str) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let input = tree.element_create(20, ElementKind::TextInput);
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
    tree.element_append_text_content(input, content);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input)
}

#[test]
fn drag_within_text_input_selects_a_range() {
    let (mut tree, input) = text_input_with("hello world");

    // フィールド先頭付近を押下し、グリフを横切って右へドラッグする。
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(60.0, 20.0);

    let (start, end) = tree
        .element_text_selection(input)
        .expect("a non-empty edit selection after dragging");
    assert!(start < end, "drag should select a non-empty range, got {start}..{end}");
}

#[test]
fn double_click_in_text_input_selects_the_word_under_the_pointer() {
    // デスクトップのダブルクリックは編集選択を単語全体へ拡張する。読み取り専用
    // SelectionArea のマルチクリック（begin_selection_at）と同じ挙動。
    let (mut tree, input) = text_input_with("hello world");

    // "hello" 内の同一地点での2回の押下で単語 0..5 へ拡張する。
    tree.on_pointer_down(15.0, 20.0);
    tree.on_pointer_up(15.0, 20.0);
    tree.on_pointer_down(15.0, 20.0);

    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 5)),
        "ダブルクリックは 'hello' を選択する",
    );
}

#[test]
fn triple_click_in_text_input_selects_the_line() {
    // 同一地点での3回目の押下は単語から行全体（段落）へ拡張する。改行がなければ
    // 行は単一行の内容全体となる。
    let (mut tree, input) = text_input_with("hello world");

    tree.on_pointer_down(15.0, 20.0);
    tree.on_pointer_up(15.0, 20.0);
    tree.on_pointer_down(15.0, 20.0);
    tree.on_pointer_up(15.0, 20.0);
    tree.on_pointer_down(15.0, 20.0);

    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 11)),
        "トリプルクリックは行全体を選択する",
    );
}

#[test]
fn single_click_in_text_input_places_a_caret_not_a_word() {
    // リグレッションガード: 単発の押下は折りたたんだキャレットを置くだけ。
    // マルチクリックの単語/行拡張が最初の押下で発火してはならない。
    let (mut tree, input) = text_input_with("hello world");

    tree.on_pointer_down(15.0, 20.0);

    assert!(
        tree.element_text_selection(input).is_none(),
        "シングルクリックは選択を折りたたんだまま（キャレット）にする",
    );
    assert!(
        tree.element_caret_byte_index(input).is_some(),
        "キャレットがフィールド内に置かれる",
    );
}

#[test]
fn a_relayout_after_a_click_does_not_move_the_caret_or_forge_a_selection() {
    // Canvas モードの根本原因: レイアウトパスは relayout のたび（スタイル変更・
    // リサイズ・選択駆動の再描画）にテキスト入力の整形内容を再構築し、その際
    // `cursor_byte_index` をテキスト末尾へ強制しつつ `selection_anchor` をクリック
    // 直後の位置に残していた。次フレームでキャレットが末尾にスナップした幻の
    // `(click..end)` 選択が読まれ、Shift+クリックは空に折りたたまれていた。
    // relayout はクリックが置いたキャレットを保持しなければならない（テキストが
    // 縮んだ場合のみクランプ）。
    let (mut tree, input) = text_input_with("hello world"); // キャレットは末尾

    tree.on_pointer_down(15.0, 20.0); // キャレットは単語の途中に着地
    let caret = tree.element_caret_byte_index(input);
    assert!(tree.element_text_selection(input).is_none(), "クリックはキャレット");

    // 定常状態の rAF フレーム同様にレイアウトをやり直させる。
    tree.element_set_style(input, &[StyleProp::FontSize(16.0)]);
    tree.render(16.0);

    assert!(
        tree.element_text_selection(input).is_none(),
        "クリック後の relayout は選択を捏造してはならない",
    );
    assert_eq!(
        tree.element_caret_byte_index(input),
        caret,
        "relayout はキャレットをテキスト末尾へ戻してはならない",
    );
}

#[test]
fn click_lands_the_caret_at_the_clicked_point_not_a_glyph_left_edge() {
    // クリックは Parley の `Cursor::from_point` でバイトオフセットへ解決され、
    // グリフのどちら側に当たったかを尊重する。以前の `byte_index_at_point` は
    // 命中クラスタの先頭を無条件に返したため、後半への押下がキャレットをグリフ
    // 先頭へスナップさせ、最後のグリフを越えた押下は末尾に届かなかった。両端を
    // 検証する: 左端の押下は最初のグリフの前 (0)、右端の押下は末尾 (len)。
    let (mut tree, input) = text_input_with("hello");

    tree.on_pointer_down(2.0, 20.0);
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(0),
        "左端の押下は最初のグリフの前に来る",
    );

    tree.on_pointer_up(2.0, 20.0);
    tree.on_pointer_down(190.0, 20.0);
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(5),
        "最後のグリフを越えた押下は左端ではなく末尾に届く",
    );
}

#[test]
fn double_click_under_touch_modality_stays_a_caret() {
    // 単語/行拡張は Mouse/Pen のジェスチャ（ADR-0104）。Touch ではダブル押下は
    // キャレットのままで、長押しの単語選択と競合しない。Mouse のダブルクリック
    // （他のテスト）は引き続き拡張する。
    let (mut tree, input) = text_input_with("hello world");

    tree.on_pointer_down_with_kind(15.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_up_with_kind(15.0, 20.0, PointerKind::Touch);
    tree.on_pointer_down_with_kind(15.0, 20.0, 0, PointerKind::Touch);

    assert!(
        tree.element_text_selection(input).is_none(),
        "Touch のダブル押下は単語へ拡張しない",
    );
}

/// フォーカス済みテキスト入力を `selectable` 段落（独立した Selection Region）の
/// 上に配置した列。(tree, input, 段落テキスト) を返す。両者ともレイアウト済み。
fn input_above_selectable_paragraph() -> (ElementTree, hayate_core::ElementId, hayate_core::ElementId)
{
    use hayate_core::FlexDirectionValue;
    let mut tree = ElementTree::new();
    let root = tree.element_create(30, ElementKind::View);
    let input = tree.element_create(31, ElementKind::TextInput);
    let region = tree.element_create(32, ElementKind::View);
    let text = tree.element_create(33, ElementKind::Text);
    // スペーサーは入力選択のフローティングツールバー（ADR-0097）から段落を
    // 離す。入力が上端固定だとツールバーは入力の下へ反転し、さもなくば段落に
    // 重なるため。
    let spacer = tree.element_create(34, ElementKind::View);
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
        input,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_set_style(
        spacer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(80.0)),
        ],
    );
    tree.element_set_style(region, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(root, input);
    tree.element_append_child(root, spacer);
    tree.element_append_child(root, region);
    tree.element_append_child(region, text);
    tree.element_append_text_content(input, "edit me");
    tree.element_set_text(text, "Hello world");
    tree.element_set_selectable(region, true);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input, text)
}

#[test]
fn starting_a_text_input_selection_clears_the_selection_area_selection() {
    let (mut tree, input, text) = input_above_selectable_paragraph();
    let (_, ty, _, th) = tree.element_layout_rect(text).unwrap();

    // まず段落 region 内の読み取り専用テキストを選択する。
    tree.on_pointer_down(2.0, ty + th / 2.0);
    tree.on_pointer_move(70.0, ty + th / 2.0);
    assert!(tree.selection().is_some(), "SelectionArea の選択が存在する");

    // 次にテキスト入力内をドラッグする: ドキュメント選択はクリアされねばならない。
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(50.0, 20.0);
    assert!(
        tree.selection().is_none(),
        "テキスト入力の選択開始は SelectionArea の選択をクリアする",
    );
    assert!(
        tree.element_text_selection(input).is_some(),
        "アクティブな選択はテキスト入力が持つ",
    );
}

#[test]
fn starting_a_selection_area_selection_clears_the_text_input_selection() {
    let (mut tree, input, text) = input_above_selectable_paragraph();

    // まずテキスト入力内の範囲を選択する。
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(50.0, 20.0);
    tree.on_pointer_up(50.0, 20.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "テキスト入力にアクティブな編集選択がある",
    );

    // 次に段落の読み取り専用テキストを選択する: 編集選択は折りたたまれる。
    let (_, ty, _, th) = tree.element_layout_rect(text).unwrap();
    tree.on_pointer_down(2.0, ty + th / 2.0);
    tree.on_pointer_move(70.0, ty + th / 2.0);
    assert!(
        tree.element_text_selection(input).is_none(),
        "SelectionArea の選択開始はテキスト入力の選択を折りたたむ",
    );
    assert!(tree.selection().is_some(), "選択は SelectionArea が持つ");
}

#[test]
fn on_key_down_backspace_edits_focused_text_input() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(1, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "hello");

    tree.on_key_down("Backspace", 0);

    assert_eq!(tree.element_get_text_content(input), "hell");
}

#[test]
fn delete_key_removes_the_grapheme_after_the_caret() {
    // Delete（前方）は EditIntent seam を通してキャレット右側の文字を削除する
    // （ADR-0103）。
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾 (5)
    tree.on_key_down("ArrowLeft", 0);
    tree.on_key_down("ArrowLeft", 0); // キャレットは 3（"lo" の前）

    tree.on_key_down("Delete", 0);
    assert_eq!(tree.element_get_text_content(input), "helo", "右側の 'l' を削除する");
    assert_eq!(tree.element_caret_byte_index(input), Some(3), "キャレットは削除位置に留まる");
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn backspace_key_removes_the_grapheme_before_the_caret() {
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾 (5)
    tree.on_key_down("ArrowLeft", 0); // キャレットは 4（"o" の前）

    tree.on_key_down("Backspace", 0);
    assert_eq!(tree.element_get_text_content(input), "helo", "左側の 'l' を削除する");
    assert_eq!(tree.element_caret_byte_index(input), Some(3), "キャレットは 'l' の開始位置へ戻る");
}

#[test]
fn backspace_and_delete_over_a_selection_remove_the_whole_range() {
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾 (5)
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT); // "lo" を選択 → (3,5)
    assert_eq!(tree.element_text_selection(input), Some((3, 5)));

    tree.on_key_down("Delete", 0);
    assert_eq!(tree.element_get_text_content(input), "hel", "1文字ではなく範囲が消える");
    assert_eq!(tree.element_caret_byte_index(input), Some(3));
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn ctrl_backspace_deletes_the_word_before_the_caret() {
    // Ctrl+Backspace（Win/Linux）はキャレット左側の単語全体を削除する
    // （1 grapheme ではない）。`selection.rs` の `prev_word` を再利用。
    let (mut tree, input) = focused_input("hello world"); // キャレットは末尾 (11)

    tree.on_key_down("Backspace", CTRL);
    assert_eq!(tree.element_get_text_content(input), "hello ", "末尾の単語が消える");
    assert_eq!(tree.element_caret_byte_index(input), Some(6), "キャレットは単語先頭に着く");
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn ctrl_delete_deletes_the_word_after_the_caret() {
    // Ctrl+Delete は `next_word` でキャレット右側の単語全体を削除し、キャレットは
    // その場に留まる。
    let (mut tree, input) = focused_input("hello world"); // キャレットは末尾 (11)
    tree.on_key_down("ArrowLeft", CTRL); // 単語単位で 6 へ
    tree.on_key_down("ArrowLeft", CTRL); // 単語単位でフィールド先頭 (0) へ
    assert_eq!(tree.element_caret_byte_index(input), Some(0));

    tree.on_key_down("Delete", CTRL);
    assert_eq!(tree.element_get_text_content(input), " world", "先頭の単語が消える");
    assert_eq!(tree.element_caret_byte_index(input), Some(0), "キャレットは削除位置に留まる");
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn alt_backspace_and_delete_delete_by_word_on_macos() {
    // macOS は Option (Alt) を単語単位の修飾キーとして使い、Win/Linux の Ctrl と
    // 同様に単語全体を削除する。
    let (mut tree, input) = focused_input("alpha beta"); // キャレットは末尾 (10)

    tree.on_key_down("Backspace", ALT);
    assert_eq!(tree.element_get_text_content(input), "alpha ", "Option+Backspace で 'beta' が消える");

    tree.on_key_down("ArrowLeft", ALT); // 単語単位でフィールド先頭 (0) へ
    tree.on_key_down("Delete", ALT);
    assert_eq!(tree.element_get_text_content(input), " ", "Option+Delete で 'alpha' が消える");
}

#[test]
fn ctrl_backspace_word_boundary_matches_the_shared_word_logic_in_mixed_text() {
    // 単語削除は `selection.rs` の単語境界に従う。CJK 連続と英語連続の境界は
    // 区切りの空白（CJK も ASCII 文字も word 文字に分類される）。"こんにちは world"
    // は2単語で、1回の Ctrl+Backspace は末尾の英単語のみ削除する。
    let (mut tree, input) = focused_input("こんにちは world"); // 15 + 1 + 5 = 21 バイト

    tree.on_key_down("Backspace", CTRL);
    assert_eq!(
        tree.element_get_text_content(input),
        "こんにちは ",
        "英単語のみ消え、CJK 単語はそのまま残る",
    );

    // 2回目の Ctrl+Backspace は空白を越えて CJK 単語を削除する。
    tree.on_key_down("Backspace", CTRL);
    assert_eq!(tree.element_get_text_content(input), "", "次に CJK 単語が消える");
}

#[test]
fn ctrl_arrow_moves_and_extends_the_caret_by_word() {
    // 単語粒度のキャレット移動と選択拡張が、同じ `on_key_down` seam を通して
    // フォーカス済みフィールドに届く（EditState の単体テストを補完する統合テスト）。
    let (mut tree, input) = focused_input("hello world"); // キャレットは末尾 (11)

    tree.on_key_down("ArrowLeft", CTRL); // "world" を越えて戻る
    assert_eq!(tree.element_caret_byte_index(input), Some(6), "'world' の先頭に着く");

    tree.on_key_down("ArrowLeft", CTRL | SHIFT); // "hello " を越えて拡張
    assert_eq!(tree.element_text_selection(input), Some((0, 6)), "選択が1単語分広がる");
}

#[test]
fn enter_in_a_multiline_field_inserts_a_newline_at_the_caret() {
    // 複数行フィールドでは Enter を末尾追加ではなくキャレット位置への改行挿入と
    // して扱う。
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_set_multiline(input, true);
    tree.element_append_text_content(input, "ab");
    tree.on_key_down("ArrowLeft", 0); // キャレットは 'a' と 'b' の間

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "a\nb", "キャレット位置に改行");
    assert_eq!(tree.element_caret_byte_index(input), Some(2), "キャレットは改行の後");
}

#[test]
fn enter_in_a_single_line_field_does_not_insert_a_newline_and_signals_submit() {
    // 既定の単一行フィールドでは Enter でテキストを変えない。KeyDown イベントが
    // アプリの submit シグナルで、TextInput は発行されない。
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "ab");

    let key_listener =
        tree.register_listener(input, hayate_core::DocumentEventKind::KeyDown);
    let text_listener =
        tree.register_listener(input, hayate_core::DocumentEventKind::TextInput);

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "ab", "改行は挿入されない");
    let deliveries = tree.poll_deliveries();
    assert!(
        deliveries.iter().any(|d| d.listener_id == key_listener
            && matches!(&d.event, hayate_core::Event::KeyDown { key, .. } if key == "Enter")),
        "Enter は KeyDown（submit シグナル）としてアプリに届く",
    );
    assert!(
        !deliveries.iter().any(|d| d.listener_id == text_listener),
        "単一行フィールドは Enter で TextInput を発行しない",
    );
}

#[test]
fn composition_end_emits_a_text_input_event_with_the_committed_value() {
    // IME 確定（compositionend）は内容を変える編集なので、DOM が `compositionend` 後に
    // `input` を発火するのと同型に、確定後の全文を載せた TextInput を続けて発行する。
    // これが無いと controlled input（value を state/signal にミラーする FW）は確定値を
    // 受け取れず、`onInput` 不発で draft が空のまま残る（Canvas 専用の text-input 追加
    // バグの根本原因）。
    let (mut tree, input) = focused_input(""); // 空フィールドに IME で確定する
    let listener = tree.register_listener(input, hayate_core::DocumentEventKind::TextInput);

    tree.on_composition_start(input, "");
    tree.on_composition_update(input, "ぎゅうにゅう");
    tree.on_composition_end(input, "ぎゅうにゅう");

    assert_eq!(tree.element_get_text_content(input), "ぎゅうにゅう");
    let deliveries = tree.poll_deliveries();
    let event = deliveries
        .iter()
        .find(|d| d.listener_id == listener)
        .map(|d| &d.event)
        .expect("compositionend must produce a TextInput delivery");
    assert!(
        matches!(event, hayate_core::Event::TextInput { text, .. } if text == "ぎゅうにゅう"),
        "the IME-commit input event must carry the full committed value, got {event:?}",
    );
}

#[test]
fn text_input_event_carries_the_full_field_value_not_just_the_typed_fragment() {
    // input イベントの value は要素の現在値全体（DOM の `input` → `target.value` と同じ）。
    // 以前は断片だけをワイヤに載せ、ホストが `element_get_text_content` で読み戻していた。
    // ADR-0069 完成（#474）で web ホストはこの読み戻しを撤去するため、配信ペイロード自体が
    // 全文を運ぶ必要がある。
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾
    let listener = tree.register_listener(input, hayate_core::DocumentEventKind::TextInput);

    tree.on_text_input(input, "X");

    let deliveries = tree.poll_deliveries();
    let event = deliveries
        .iter()
        .find(|d| d.listener_id == listener)
        .map(|d| &d.event)
        .expect("a TextInput delivery");
    assert!(
        matches!(event, hayate_core::Event::TextInput { text, .. } if text == "helloX"),
        "the input event must carry the full field value, got {event:?}",
    );
}

#[test]
fn enter_in_a_multiline_field_replaces_the_selection() {
    // replace-on-type: 範囲選択上の Enter は範囲を消しその位置に改行を挿入する。
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_set_multiline(input, true);
    tree.element_append_text_content(input, "hello"); // キャレットは末尾 (5)
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT); // "lo" を選択 → (3,5)
    assert_eq!(tree.element_text_selection(input), Some((3, 5)));

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "hel\n", "範囲が改行に置換される");
    assert_eq!(tree.element_caret_byte_index(input), Some(4));
    assert!(tree.element_text_selection(input).is_none());
}

// ── 複数行の垂直移動 + 表示行単位の Home/End ─────────────────
// ↑/↓ は sticky な goal column を保ちつつ表示行間を移動し、Home/End は表示行の
// 端へスナップする。Parley の行ジオメトリが必要なため先にレイアウトする。幅は
// 広めにし、ハード改行のケースがソフトラップしないようにする。

/// `content` を保持しレイアウト済みのフォーカス済み複数行テキスト入力。`width`
/// でソフトラップを強制でき、`content` はハード行用に `\n` を含められる。
fn multiline_input(content: &str, width: f32) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let input = tree.element_create(40, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(width.max(200.0), 200.0);
    tree.element_set_multiline(input, true);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(width)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_append_text_content(input, content);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input)
}

fn move_vertical(d: Direction) -> EditIntent {
    EditIntent::Move {
        granularity: Granularity::Grapheme,
        direction: d,
    }
}

#[test]
fn arrow_up_down_moves_between_display_lines_at_the_same_column() {
    // 同一のハード行が2つ。2行目の末尾から ↑ は1行目の同じ列（その末尾）に着き、
    // ↓ は2行目の末尾へ戻る。
    let (mut tree, input) = multiline_input("abcdef\nabcdef", 400.0); // キャレットは 13
    assert_eq!(tree.element_caret_byte_index(input), Some(13));

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(6),
        "↑ は上の行の末尾（同じ列）に着く",
    );

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Down)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(13),
        "↓ は下の行の末尾へ戻る",
    );
}

#[test]
fn vertical_motion_keeps_the_goal_column_across_a_short_line() {
    // sticky goal column: 長い行の末尾から ↑ で短い行 ("hi") を通り、さらに ↑ で
    // 別の長い行へ。短い行を越えても列が保たれるため、最終キャレットは短い行が
    // クランプした位置（列2付近）ではなく長い行の末尾 (5) になる。
    let (mut tree, input) = multiline_input("world\nhi\nworld", 400.0); // キャレットは 14
    assert_eq!(tree.element_caret_byte_index(input), Some(14));

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(8),
        "↑ で短い行に乗るとその末尾にクランプされる",
    );

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(5),
        "再度 ↑ で長い行の元の列へ戻る（goal が保たれている）",
    );
}

#[test]
fn single_line_arrow_up_down_jumps_to_the_field_ends() {
    // 単一行フィールドには行がないため、↑ はフィールド先頭、↓ はフィールド末尾へ
    // 飛ぶ（Chromium `<input>`）。純粋な EditState seam で解決される。
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾 (5)

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(tree.element_caret_byte_index(input), Some(0), "↑ → フィールド先頭");

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Down)));
    assert_eq!(tree.element_caret_byte_index(input), Some(5), "↓ → フィールド末尾");
}

#[test]
fn multiline_home_end_snap_to_the_display_line_ends() {
    // 複数行フィールドの Home/End はフィールド端ではなく表示行の端へ移動する。
    // 3行目のキャレット → Home はその行の先頭 (9)（フィールド先頭 0 ではない）、
    // End はその末尾 (14) に着く。
    let (mut tree, input) = multiline_input("world\nhi\nworld", 400.0); // キャレットは 14

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(9),
        "Home → フィールド先頭ではなく現在の表示行の先頭",
    );

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(14), "End → 表示行の末尾");
}

#[test]
fn shift_arrow_down_extends_the_selection_across_lines() {
    // Shift+↑/↓ は anchor を保ったまま選択を1行分拡張する。
    let (mut tree, input) = multiline_input("abcdef\nabcdef", 400.0); // キャレットは 13

    // キャレットを1行目の末尾へ（拡張の anchor 点）。
    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(tree.element_caret_byte_index(input), Some(6));

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: Direction::Down,
        },
    ));
    assert_eq!(
        tree.element_text_selection(input),
        Some((6, 13)),
        "Shift+↓ は1行目の末尾から2行目の末尾まで選択する（行をまたぐ）",
    );
}

#[test]
fn on_key_down_arrow_up_moves_the_caret_up_a_line() {
    // 生のキー経路は素の ↑ を垂直移動にマップするため、複数行フィールドはアダプタ
    // を通さずキャレットを上の行へ移動する。
    let (mut tree, input) = multiline_input("abcdef\nabcdef", 400.0); // キャレットは 13

    tree.on_key_down("ArrowUp", 0);

    assert_eq!(tree.element_caret_byte_index(input), Some(6), "↑ キーで1行上へ移動した");
}

#[test]
fn vertical_motion_follows_soft_wrapped_lines() {
    // ハード改行のない狭いフィールドは複数の表示行へソフトラップする。末尾からの
    // ↑ はキャレットを上の視覚行へ移し、y がおよそ1行分下がる。ラップのジオメトリ
    // が移動を駆動していることを示す。
    let (mut tree, input) = multiline_input("aaaa bbbb cccc dddd eeee", 70.0);

    let before = tree
        .element_character_bounds(input)
        .expect("caret bounds before moving");

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));

    let after = tree
        .element_character_bounds(input)
        .expect("caret bounds after moving");
    assert!(
        after.y < before.y - 1.0,
        "↑ がキャレットを上のラップ行へ移した (y {} → {})",
        before.y,
        after.y,
    );
}

#[test]
fn on_composition_end_commits_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(3, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_append_text_content(input, "abc");
    tree.on_composition_start(input, "DEF");
    tree.on_composition_end(input, "愛");

    assert_eq!(tree.element_get_text_content(input), "abc愛");
}

#[test]
fn composition_format_ranges_reach_core_as_underlines() {
    // EditContext `textformatupdate` → wire → core: preedit 更新と共に届く節の
    // フォーマット範囲が、表示テキストの下線範囲として現れる（ADR-0102）。確定済み
    // の "abc"（3バイト）がオフセットをずらす。
    let mut tree = ElementTree::new();
    let input = tree.element_create(7, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_append_text_content(input, "abc");
    tree.on_composition_start(input, "ぎゅうにゅう");

    let clauses = CompositionClause::from_wire(&[0, 9, 1, 9, 18, 0]);
    tree.on_composition_update_formatted(input, "ぎゅうにゅう", clauses);

    assert_eq!(
        tree.element_composition_underlines(input),
        vec![
            (3, 12, CompositionUnderline::Thick),
            (12, 21, CompositionUnderline::Thin),
        ],
    );

    // composition を確定すると下線がクリアされる。
    tree.on_composition_end(input, "牛乳");
    assert!(tree.element_composition_underlines(input).is_empty());
}

#[test]
fn on_text_input_appends_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(4, ElementKind::TextInput);
    tree.set_root(input);

    tree.on_text_input(input, "x");

    assert_eq!(tree.element_get_text_content(input), "x");
}

/// アクティブな preedit を持つフォーカス済み・レイアウト済みテキスト入力。
/// その draw ops を返す。
fn render_with_preedit(
    preedit: &str,
    clauses: Vec<CompositionClause>,
) -> Vec<hayate_core::DrawOp> {
    let mut tree = ElementTree::new();
    let input = tree.element_create(30, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(300.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_focus(input);
    tree.element_set_preedit_with_clauses(input, preedit, clauses);
    tree.render(0.0);

    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// composition の下線矩形: 高さ ≤3px・幅 ≥5px で、背の高い細線キャレットと区別
/// できる。(x, width, height) を左から右へソートして返す。
fn underline_rects(ops: &[hayate_core::DrawOp]) -> Vec<(f32, f32, f32)> {
    let mut rects: Vec<(f32, f32, f32)> = ops
        .iter()
        .filter_map(|op| match op {
            hayate_core::DrawOp::FillRect { x, width, height, .. }
                if *height <= 3.0 && *width >= 5.0 =>
            {
                Some((*x, *width, *height))
            }
            _ => None,
        })
        .collect();
    rects.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    rects
}

#[test]
fn unformatted_preedit_draws_a_single_underline() {
    // 変換前: composition 全体にまたがる細線下線が1本。
    let ops = render_with_preedit("ぎゅうにゅう", Vec::new());
    let rects = underline_rects(&ops);
    assert_eq!(rects.len(), 1, "preedit 全体に下線が1本");
    assert!(rects[0].1 > 10.0, "合成テキスト全体にまたがる");
}

#[test]
fn clause_split_draws_a_thick_and_thin_underline() {
    // 変換中はアクティブな節が太線、残りが細線（ADR-0102）。
    let ops = render_with_preedit(
        "ぎゅうにゅう",
        vec![
            CompositionClause { start: 0, end: 9, underline: CompositionUnderline::Thick },
            CompositionClause { start: 9, end: 18, underline: CompositionUnderline::Thin },
        ],
    );
    let rects = underline_rects(&ops);
    assert_eq!(rects.len(), 2, "節ごとに下線が1本");
    let (thick, thin) = (rects[0], rects[1]);
    assert!(
        thick.2 > thin.2,
        "アクティブな節の下線は確定済みより太い ({thick:?} vs {thin:?})",
    );
}

#[test]
fn committing_the_composition_removes_the_underline() {
    let ops = render_with_preedit("ぎゅう", Vec::new());
    assert_eq!(underline_rects(&ops).len(), 1);

    // 確定後 preedit は空: composition の下線は残らない。
    let ops_after = render_with_preedit("", Vec::new());
    assert!(underline_rects(&ops_after).is_empty());
}

#[test]
fn element_character_bounds_available_after_layout() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(5, ElementKind::TextInput);
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
    tree.element_append_text_content(input, "hi");
    tree.render(0.0);

    let bounds = tree
        .element_character_bounds(input)
        .expect("character bounds after layout");
    assert!(bounds.width > 0.0);
    assert!(bounds.height > 0.0);
}

#[test]
fn element_character_bounds_respects_padding() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(6, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::PaddingLeft(Dimension::px(12.0)),
            StyleProp::PaddingTop(Dimension::px(8.0)),
            StyleProp::FontSize(13.0),
        ],
    );
    tree.element_append_text_content(input, "hi");
    tree.element_focus(input);
    let cursor_rect = {
        let sg = tree.render(0.0);
        sg.iter().find_map(|(_, n)| {
            if let hayate_core::NodeKind::Rect {
                x,
                y,
                width,
                height,
                corner_radius,
                ..
            } = &n.kind
            {
                if *width <= 2.0 && *height > 10.0 && *corner_radius == 0.0 {
                    Some((*x, *y))
                } else {
                    None
                }
            } else {
                None
            }
        })
    };

    let bounds = tree
        .element_character_bounds(input)
        .expect("character bounds with padding");
    if let Some((cursor_x, cursor_y)) = cursor_rect {
        assert!(
            (bounds.x - cursor_x).abs() < 0.5,
            "IME bounds x should match canvas cursor x"
        );
        assert!(
            (bounds.y - cursor_y).abs() < 0.5,
            "IME bounds y should match canvas cursor y"
        );
    }
    assert!(
        bounds.x >= 12.0,
        "IME bounds x should be inset by padding-left, got x={}",
        bounds.x
    );
}

// ── クリップボードのキー経路（ADR-0103） ──────────────────────────────────
// Ctrl/Cmd+A/C/X/V は矢印と同じ EditIntent seam を通してフォーカス済みテキスト
// 入力に届く。主修飾キーは Win/Linux では Ctrl（アダプタのキーマップで Cmd が
// これにマップされる）。core テストは Ctrl ビットを直接駆動する。
const CTRL: u32 = 2; // MODIFIER_CTRL（proto/spec のワイヤ契約）。

/// 書き込みを記録し事前設定の読み取り値を返す `Clipboard` のダブル。Platform
/// Adapter 境界を越えた内容をテストで検証できる（`selection_toolbar.rs` の
/// ハーネスと同様）。
#[derive(Default, Clone)]
struct FakeClipboard {
    writes: Rc<RefCell<Vec<String>>>,
    read: Rc<RefCell<Option<String>>>,
}

impl Clipboard for FakeClipboard {
    fn write_text(&self, text: &str) {
        self.writes.borrow_mut().push(text.to_string());
    }
    fn read_text(&self) -> Option<String> {
        self.read.borrow().clone()
    }
}

#[test]
fn ctrl_a_selects_all_in_the_focused_text_input() {
    // フォーカス済みテキスト入力は Ctrl/Cmd+A を SelectAll EditIntent として受け、
    // 内容全体を選択する（ADR-0103）。
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾 (5) で折りたたみ

    tree.on_key_down("a", CTRL);

    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 5)),
        "Ctrl+A はフィールド内容全体を選択する",
    );
}

#[test]
fn ctrl_c_copies_the_text_input_selection_to_the_clipboard() {
    // フォーカス済みテキスト入力上の Ctrl/Cmd+C は選択テキストを Platform Adapter
    // のクリップボードへ書き込み、選択はそのまま残す（Chromium）。
    let (mut tree, input) = focused_input("hello"); // キャレットは末尾
    let clipboard = FakeClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));
    tree.on_key_down("a", CTRL); // "hello" を選択

    tree.on_key_down("c", CTRL);

    assert_eq!(clipboard.writes.borrow().as_slice(), &["hello".to_string()]);
    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 5)),
        "Copy は選択をそのまま残す",
    );
}

#[test]
fn ctrl_x_cuts_the_text_input_selection() {
    // Ctrl/Cmd+X は選択をクリップボードへ書き込みフィールドから削除し、キャレット
    // を切り取り位置へ折りたたむ（ADR-0097, ADR-0103）。
    let (mut tree, input) = focused_input("hello world"); // キャレットは末尾
    let clipboard = FakeClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));
    // 末尾の "world" を選択: 末尾から Shift+Left を5回。
    for _ in 0..5 {
        tree.on_key_down("ArrowLeft", SHIFT);
    }
    assert_eq!(tree.element_text_selection(input), Some((6, 11)));

    tree.on_key_down("x", CTRL);

    assert_eq!(clipboard.writes.borrow().as_slice(), &["world".to_string()]);
    assert_eq!(
        tree.element_get_text_content(input),
        "hello ",
        "Cut は選択範囲だけを正確に削除する",
    );
    assert!(
        tree.element_text_selection(input).is_none(),
        "Cut はキャレットを折りたたむ",
    );
}

#[test]
fn ctrl_v_pastes_clipboard_text_replacing_the_selection() {
    // Ctrl/Cmd+V は（同期の）クリップボード読み取りでテキストを取得して挿入し、
    // 選択範囲があれば置換する（replace-on-type, ADR-0097）。
    let (mut tree, input) = focused_input("hello world"); // キャレットは末尾
    let clipboard = FakeClipboard::default();
    *clipboard.read.borrow_mut() = Some("X".to_string());
    tree.set_clipboard(Box::new(clipboard.clone()));
    // 末尾の "world" を選択する。
    for _ in 0..5 {
        tree.on_key_down("ArrowLeft", SHIFT);
    }

    tree.on_key_down("v", CTRL);

    assert_eq!(
        tree.element_get_text_content(input),
        "hello X",
        "paste は選択範囲をクリップボードのテキストで置換する",
    );
}

#[test]
fn ctrl_v_pastes_at_a_collapsed_caret_in_an_empty_field() {
    // キーボードの paste はフォーカス済みフィールドを直接対象とするため、選択が
    // なくても（空フィールドの折りたたみキャレット）動作する。ツールバーの選択前提
    // の paste では届かなかったケース。
    let (mut tree, input) = focused_input(""); // 空、キャレットは 0
    let clipboard = FakeClipboard::default();
    *clipboard.read.borrow_mut() = Some("pasted".to_string());
    tree.set_clipboard(Box::new(clipboard.clone()));

    tree.on_key_down("v", CTRL);

    assert_eq!(tree.element_get_text_content(input), "pasted");
}
