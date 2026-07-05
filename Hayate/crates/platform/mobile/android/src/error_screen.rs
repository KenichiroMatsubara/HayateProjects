//! Miharashi Android ホストが boot 失敗を画面に出すための最小エラー画面。
//!
//! Web ホスト（`@miharashi/host-web` の built-in error panel）と対称の保証：boot 失敗
//! （バンドル取得失敗・protocol version 不一致）を、ログだけでなく画面にも出す。アプリ側
//! （consumer）の実装に依存せず、ホスト自身（`app_tsubame::run`）が必ず描画する——さもないと
//! 「エラーメッセージなく落ちる」ように見える（#530 / Web ホスト側と同じ教訓）。
//!
//! `hayate-core` の要素 API のみを使う（NDK 非依存）ため、ツリーの中身をホスト上でテストできる。

use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, FlexDirectionValue,
    JustifyValue, StyleProp,
};

/// エラー画面の安定した要素 id（`scene_demo` の id 帯と衝突しない専用レンジ）。
const ROOT_ID: u64 = 101;
const TEXT_ID: u64 = 102;

/// 背景色（Web ホストの built-in error panel と揃えた濃紺）。
const BACKGROUND: Color = Color::new(0.043, 0.063, 0.125, 1.0);
/// テキスト色（Web ホストの built-in error panel と揃えた淡赤）。
const TEXT_COLOR: Color = Color::new(0.988, 0.647, 0.647, 1.0);

/// 明示エラーメッセージを画面いっぱいに表示するツリーを組む。boot 失敗時、consumer が
/// 何もしなくてもホストがこれを描画する（Web ホストの built-in error panel と対称）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn build_error_tree(message: &str) -> ElementTree {
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
            StyleProp::Padding(Dimension::px(32.0)),
            StyleProp::BackgroundColor(BACKGROUND),
        ],
    );

    let text = tree.element_create(TEXT_ID, ElementKind::Text);
    tree.element_append_child(root, text);
    tree.element_set_style(
        text,
        &[
            StyleProp::MaxWidth(Dimension::percent(100.0)),
            StyleProp::FontSize(20.0),
            StyleProp::Color(TEXT_COLOR),
        ],
    );
    tree.element_set_text(text, message);

    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::ElementId;

    #[test]
    fn shows_the_given_message() {
        let tree = build_error_tree("dev-server からのバンドル取得に失敗しました");
        assert_eq!(
            tree.element_get_text(ElementId::from_u64(TEXT_ID)),
            "dev-server からのバンドル取得に失敗しました"
        );
    }

    #[test]
    fn root_fills_the_viewport_with_the_error_background() {
        let mut tree = build_error_tree("boom");
        tree.set_viewport(400.0, 800.0);
        tree.render(0.0);

        let root = ElementId::from_u64(ROOT_ID);
        let visual = tree
            .element_effective_visual(root)
            .expect("root has an effective visual");
        assert_eq!(visual.background_color, Some(BACKGROUND));
    }

    #[test]
    fn renders_without_panicking_at_a_range_of_viewport_sizes() {
        for (w, h) in [(320.0, 480.0), (1080.0, 2400.0), (100.0, 100.0)] {
            let mut tree = build_error_tree("エラーメッセージが長い場合でも折り返して読めること");
            tree.set_viewport(w, h);
            tree.render(0.0);
        }
    }
}
