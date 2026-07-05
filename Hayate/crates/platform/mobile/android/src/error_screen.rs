//! Miharashi Android ホストが boot 失敗を画面に出すための最小エラー画面。
//!
//! Web ホスト（`@miharashi/host-web` の built-in error panel）と対称の保証：boot 失敗
//! （バンドル取得失敗・protocol version 不一致）を、ログだけでなく画面にも出す。アプリ側
//! （consumer）の実装に依存せず、ホスト自身（`app_tsubame::run`）が必ず描画する——さもないと
//! 「エラーメッセージなく落ちる」ように見える（#530 / Web ホスト側と同じ教訓）。
//!
//! `hayate-core` の要素 API のみを使う（NDK 非依存）ため、ツリーの中身をホスト上でテストできる。
//!
//! Web ホストはこれを生 DOM/CSS で描く（`Miharashi/host-web/src/index.ts` の
//! `renderBuiltinErrorPanel`）のに対し、こちらは Hayate 自身の要素ツリー→GPU パイプラインで
//! 描く——実装を分けているのは単に Rust/TS が実行時コードを共有できないからだけではない。
//! Web の生 DOM は Hayate/WebGPU の初期化自体が失敗しても描画できる「潰れない土台」だが、
//! Android はまだそれに相当する GPU 非依存の表示手段を持たない（Kotlin 側に JNI 経由で
//! Toast/TextView を生やす別作業が要る、QR スキャナの JNI ブリッジと同種）。よって
//! `init_gpu_surface` 自体が失敗した場合はこの画面も出せず、ログにのみ残る——既知の非対称。
//! 色・文言は「見た目の仕様」として揃えているが、実行時コードは共有できないため Web 側の
//! 値を手で複製し、下記テストで固定する（`protocol_handshake.rs` と同じ方針）。

use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, FlexDirectionValue,
    JustifyValue, StyleProp,
};

/// エラー画面の安定した要素 id（`scene_demo` の id 帯と衝突しない専用レンジ）。
const ROOT_ID: u64 = 101;
const TEXT_ID: u64 = 102;

/// 背景色。Web ホストの built-in error panel（`background:#0b1020`）と同じ値を複製する
/// （下記 `web_colors_match_the_documented_hex_values` で固定）。
const BACKGROUND: Color = Color::new(0.043, 0.063, 0.125, 1.0);
/// テキスト色。Web ホストの built-in error panel（`color:#fca5a5`）と同じ値を複製する。
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

    /// `#rrggbb` を `Color`（sRGB バイト値、0.0..=1.0）に変換する。テスト専用の逆算ヘルパー。
    fn from_hex(hex: &str) -> Color {
        let bytes = u32::from_str_radix(hex.trim_start_matches('#'), 16).expect("valid hex");
        let r = ((bytes >> 16) & 0xff) as f64 / 255.0;
        let g = ((bytes >> 8) & 0xff) as f64 / 255.0;
        let b = (bytes & 0xff) as f64 / 255.0;
        Color::new(r, g, b, 1.0)
    }

    #[test]
    fn web_colors_match_the_documented_hex_values() {
        // Web ホスト（`Miharashi/host-web/src/index.ts` の `renderBuiltinErrorPanel`）が使う
        // `background:#0b1020` / `color:#fca5a5` を手複製した定数が、実際にその hex 値と一致する
        // ことを固定する（`protocol_handshake.rs` の wire 契約 pin テストと同じ方針）。
        let background_from_hex = from_hex("#0b1020");
        assert!(
            (BACKGROUND.r - background_from_hex.r).abs() < 0.002
                && (BACKGROUND.g - background_from_hex.g).abs() < 0.002
                && (BACKGROUND.b - background_from_hex.b).abs() < 0.002,
            "BACKGROUND {BACKGROUND:?} should match #0b1020 ({background_from_hex:?})"
        );

        let text_from_hex = from_hex("#fca5a5");
        assert!(
            (TEXT_COLOR.r - text_from_hex.r).abs() < 0.002
                && (TEXT_COLOR.g - text_from_hex.g).abs() < 0.002
                && (TEXT_COLOR.b - text_from_hex.b).abs() < 0.002,
            "TEXT_COLOR {TEXT_COLOR:?} should match #fca5a5 ({text_from_hex:?})"
        );
    }

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
