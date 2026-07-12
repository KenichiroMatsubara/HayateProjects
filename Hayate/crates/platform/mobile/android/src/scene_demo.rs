//! 実機確認用の最小の操作可能な要素ツリー（ADR-0087）。
//!
//! ビューポート中央のボタンは押下中に `:active` 背景が反転し、タップで
//! `ElementTree -> SceneGraph -> Vello` パイプとタッチ配線がエンドツーエンドで
//! ピクセルを変えることを目に見える形で確認できる。
//!
//! ビルダーは `hayate-core` の要素 API のみを使う（NDK 非依存）ため、ツリーの
//! 操作可能性をホスト上でテストできる継ぎ目になる。`app.rs` はビューポートを
//! サイズ設定して毎フレーム描画する薄いグルー。

use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, FlexDirectionValue,
    JustifyValue, PositionValue, PseudoState, StyleProp,
};

/// デモツリーの安定した要素 id（実機ログから参照できるよう、`hayate-adapter-web`
/// が JS 側で id を割り当てるのを踏襲）。
pub const ROOT_ID: u64 = 1;
pub const BUTTON_ID: u64 = 2;
pub const TEXT_INPUT_ID: u64 = 3;
/// 読み取り専用 SelectionArea のフローティングツールバーを示す `selectable`
/// 段落（Text 子が IFC）（ADR-0097）。
pub const PARAGRAPH_ID: u64 = 4;
pub const PARAGRAPH_TEXT_ID: u64 = 5;

/// selectable デモ段落の文言。末尾の絵文字は skia 選択時のカラーグリフ実機確認用
/// （issue #802・ADR-0146 §4: `paints_color_glyphs()` = true な skia がカラーで描けるか、
/// 先頭語("Drag")に依存する既存テスト・タッチ座標には影響しない位置に追加した）。
pub const PARAGRAPH_TEXT: &str = "Drag to select this text 🎉😀🚀";

/// 非押下時のボタン背景。
pub const BUTTON_IDLE: Color = Color::new(0.16, 0.45, 0.92, 1.0);
/// `:active`（押下時）のボタン背景。指の下で目に見えて反転する。
pub const BUTTON_ACTIVE: Color = Color::new(0.92, 0.35, 0.16, 1.0);

/// デモツリーを構築する。ビューポート全体の flex column で、押下中に色が反転する
/// ボタンと、ソフトキーボードを受ける IME ブリッジ用の text-input を中央に配置する
/// （ADR-0094）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn build_demo_tree() -> ElementTree {
    let mut tree = ElementTree::new();

    let root = tree.element_create(ROOT_ID, ElementKind::View);
    tree.set_root(root);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::Gap(Dimension::px(24.0)),
        ],
    );

    let button = tree.element_create(BUTTON_ID, ElementKind::Button);
    tree.element_append_child(root, button);
    tree.element_set_style(
        button,
        &[
            StyleProp::Width(Dimension::px(220.0)),
            StyleProp::Height(Dimension::px(96.0)),
            StyleProp::BorderRadius(16.0),
            StyleProp::BackgroundColor(BUTTON_IDLE),
        ],
    );
    tree.element_set_pseudo_style(
        button,
        PseudoState::Active,
        &[StyleProp::BackgroundColor(BUTTON_ACTIVE)],
    );

    let input = tree.element_create(TEXT_INPUT_ID, ElementKind::TextInput);
    tree.element_append_child(root, input);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(260.0)),
            StyleProp::Height(Dimension::px(56.0)),
            StyleProp::BorderRadius(8.0),
            StyleProp::BorderWidth(2.0),
            StyleProp::BorderColor(Color::new(0.4, 0.4, 0.45, 1.0)),
            StyleProp::BackgroundColor(Color::WHITE),
            StyleProp::FontSize(20.0),
            StyleProp::Color(Color::BLACK),
        ],
    );

    // ビューポート上部に固定した selectable 段落（absolute なので中央の
    // ボタン/入力カラムを乱さない）。ドラッグするとコア描画の Material 選択
    // ツールバー（Copy / Select All）が出る（ADR-0097）。
    let paragraph = tree.element_create(PARAGRAPH_ID, ElementKind::View);
    tree.element_append_child(root, paragraph);
    tree.element_set_style(
        paragraph,
        &[
            StyleProp::Position(PositionValue::Absolute),
            StyleProp::Top(Dimension::px(24.0)),
            StyleProp::Left(Dimension::px(24.0)),
            StyleProp::Width(Dimension::px(320.0)),
        ],
    );
    tree.element_set_selectable(paragraph, true);

    let paragraph_text = tree.element_create(PARAGRAPH_TEXT_ID, ElementKind::Text);
    tree.element_append_child(paragraph, paragraph_text);
    tree.element_set_style(
        paragraph_text,
        &[
            StyleProp::Width(Dimension::px(320.0)),
            StyleProp::FontSize(20.0),
            StyleProp::Color(Color::new(0.1, 0.1, 0.12, 1.0)),
        ],
    );
    tree.element_set_text(paragraph_text, PARAGRAPH_TEXT);

    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::ElementId;

    #[test]
    fn demo_button_starts_at_the_idle_color() {
        let tree = build_demo_tree();
        let button = ElementId::from_u64(BUTTON_ID);
        let visual = tree
            .element_effective_visual(button)
            .expect("button has an effective visual");
        assert_eq!(visual.background_color, Some(BUTTON_IDLE));
    }

    // 押下で実効背景が `:active` 色に反転し、離すと戻る。実機タップで可視化したい
    // エンドツーエンドの挙動。
    #[test]
    fn pressing_the_button_flips_its_background() {
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        let button = ElementId::from_u64(BUTTON_ID);

        // 400×800 で column 中央: ボタンは x 90..310, y 312..408、中心は (200, 360)。
        tree.on_pointer_down(200.0, 360.0);
        assert_eq!(
            tree.element_effective_visual(button)
                .expect("button visual")
                .background_color,
            Some(BUTTON_ACTIVE),
            "press should flip to the :active background"
        );

        tree.on_pointer_up(200.0, 360.0);
        assert_eq!(
            tree.element_effective_visual(button)
                .expect("button visual")
                .background_color,
            Some(BUTTON_IDLE),
            "release should restore the idle background"
        );
    }

    // text-input のタップでフォーカスされる。グルーがソフトキーボードを表示し
    // GameTextInput をそこへ流す前提条件。
    #[test]
    fn tapping_the_text_input_focuses_it() {
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        // text-input はボタン下の y 432..488、中心 (200, 460)。
        tree.on_pointer_down(200.0, 460.0);
        tree.on_pointer_up(200.0, 460.0);

        assert_eq!(
            tree.focused_element(),
            Some(ElementId::from_u64(TEXT_INPUT_ID)),
            "tapping the text-input should focus it"
        );
    }

    // selectable 段落をドラッグするとコア描画のフローティングツールバー
    // （Copy / Select All）が出る。読み取り専用 SelectionArea の UI（ADR-0097）。
    #[test]
    fn dragging_the_paragraph_shows_the_selection_toolbar() {
        use hayate_core::ToolbarAction;
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        // 段落は absolute (24, 24)。先頭グリフを Touch でドラッグする。
        // フローティングツールバーは Touch 限定の UI（ADR-0104）。
        tree.on_pointer_down_with_kind(28.0, 32.0, 0, hayate_core::PointerKind::Touch);
        tree.on_pointer_move(120.0, 32.0);
        tree.on_pointer_up(120.0, 32.0);

        let toolbar = tree
            .selection_toolbar()
            .expect("the selection toolbar is shown after dragging the paragraph");
        assert_eq!(
            toolbar.actions(),
            vec![ToolbarAction::Copy, ToolbarAction::SelectAll],
        );
    }

    // 段落の長押しは Material の単語選択を開始し、ドラッグハンドルとツールバーを
    // 出す。モバイル風の選択（ADR-0097）。
    #[test]
    fn long_pressing_the_paragraph_selects_a_word_with_handles() {
        let mut tree = build_demo_tree();
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        // 段落は absolute (24, 24)。先頭の単語を長押しする。
        tree.on_long_press(30.0, 32.0);

        let selected = tree
            .selected_text()
            .expect("long-press selects a word on the paragraph");
        assert!(
            PARAGRAPH_TEXT.starts_with(&selected),
            "the long-pressed word ({selected:?}) is the paragraph's first word",
        );

        let handles = tree
            .selection_handles()
            .expect("word selection raises drag handles");
        assert!(
            handles.start.knob_x < handles.end.knob_x,
            "a left-to-right word puts the start handle left of the end handle",
        );
        assert!(
            tree.selection_toolbar().is_some(),
            "word selection raises the floating toolbar too",
        );
    }
}
