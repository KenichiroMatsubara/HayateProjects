//! Canvas Mode の「chrome」味付け定数（スクロールバーオーバーレイ、タッチ
//! インジケータ、選択ハイライト/ハンドル/ツールバー、IME 下線、プレースホルダ
//! アルファ）の実行時上書き可能なコピー。
//!
//! [`scene_build`](crate::element::scene_build) と
//! [`selection_chrome`](crate::element::selection_chrome) の名前付き `const` が
//! 唯一の正となるデフォルト値で、[`Default`] がそれを読むため数値はここに再掲しない。
//! 開発ビルドでは実行時に値を上書きでき（Platform Adapter が serde を持ち `tuning.json`
//! を解析）、JSON を編集して F5 を押すだけで再ビルドなしに Chromium/Android に合わせて
//! 較正できる。本番は上書きを持たないため各フィールドは const と等しく、読み出しは
//! [`ElementTree`](crate::element::tree::ElementTree) からの単純なフィールドロード
//! （旧来の `const` 参照に対し性能コストなし）。
//!
//! スコープ（v1）: 上書き可能なのは描画時の視覚値のみ。レイアウト/ヒットテスト
//! ジオメトリ（ハンドルのヒット半径、ツールバー間隔、ラベル advance）と
//! インジケータのフェードタイミングは const のまま（ツリーを受け取らない関数が読む）で、
//! 変更には再ビルドが要る。

use crate::element::scene_build;
use crate::element::selection_chrome;
use crate::Color;

/// 実行時に上書き可能な chrome 定数。モジュールドキュメント参照。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeTuning {
    // ── スクロールバーオーバーレイ（Mouse/Pen）、ADR-0110 ──
    pub scrollbar_thickness: f32,
    pub scrollbar_track_margin: f32,
    pub scrollbar_min_thumb_length: f32,
    pub scrollbar_thumb_color: Color,
    pub scrollbar_thumb_opacity: f32,
    // ── タッチの一時インジケータ ──
    pub scrollbar_indicator_thickness: f32,
    pub scrollbar_indicator_color: Color,
    pub scrollbar_indicator_opacity: f32,
    // ── 選択ハイライトの色味（Chromium `::selection`）、ADR-0097 ──
    pub selection_highlight_color: [f32; 4],
    // ── IME 変換の下線、ADR-0102 ──
    pub composition_underline_thin: f32,
    pub composition_underline_thick: f32,
    // ── プレースホルダ文字のミュート、ADR-0102 ──
    pub placeholder_alpha: f64,
    // ── フローティング選択ツールバー（Android ネイティブ）、ADR-0097 ──
    //
    // 視覚値（高さ・ラベルフォントサイズ・ボタン左右パディング・選択との gap・
    // ディバイダ色/幅・elevation）は名前付きフィールドで `tuning.json` から再ビルド
    // 不要に上書きできる。パネル背景/ラベルの*色*とハンドル色だけは引き続き切替式の
    // `SelectionChromeStyle`（Material/Cupertino）テーマが所有し、この上書きは触れない
    // （ADR-0097）。
    pub toolbar_corner_radius: f32,
    pub toolbar_height: f32,
    pub toolbar_label_font_size: f32,
    pub toolbar_button_pad_x: f32,
    pub toolbar_gap: f32,
    /// ボタン間 Material ディバイダ（新規 affordance なので色も tunable）。
    pub toolbar_divider_color: [f32; 4],
    pub toolbar_divider_width: f32,
    /// パネルの Material elevation（drop shadow）。既存 box-shadow lowering で描く。
    pub toolbar_elevation_offset_y: f32,
    pub toolbar_elevation_blur: f32,
    pub toolbar_elevation_spread: f32,
    pub toolbar_shadow_color: [f32; 4],
}

impl Default for ChromeTuning {
    fn default() -> Self {
        // 正の const を写すだけでリテラルは再掲しない。const ブロックを
        // デフォルト値の唯一の出所に保つ。
        Self {
            scrollbar_thickness: scene_build::SCROLLBAR_THICKNESS,
            scrollbar_track_margin: scene_build::SCROLLBAR_TRACK_MARGIN,
            scrollbar_min_thumb_length: scene_build::SCROLLBAR_MIN_THUMB_LENGTH,
            scrollbar_thumb_color: scene_build::SCROLLBAR_THUMB_COLOR,
            scrollbar_thumb_opacity: scene_build::SCROLLBAR_THUMB_OPACITY,
            scrollbar_indicator_thickness: scene_build::SCROLLBAR_INDICATOR_THICKNESS,
            scrollbar_indicator_color: scene_build::SCROLLBAR_INDICATOR_COLOR,
            scrollbar_indicator_opacity: scene_build::SCROLLBAR_INDICATOR_OPACITY,
            selection_highlight_color: scene_build::SELECTION_HIGHLIGHT_COLOR,
            composition_underline_thin: scene_build::COMPOSITION_UNDERLINE_THIN,
            composition_underline_thick: scene_build::COMPOSITION_UNDERLINE_THICK,
            placeholder_alpha: scene_build::PLACEHOLDER_ALPHA,
            toolbar_corner_radius: selection_chrome::TOOLBAR_CORNER_RADIUS,
            toolbar_height: selection_chrome::TOOLBAR_HEIGHT,
            toolbar_label_font_size: selection_chrome::TOOLBAR_LABEL_FONT_SIZE,
            toolbar_button_pad_x: selection_chrome::TOOLBAR_BUTTON_PAD_X,
            toolbar_gap: selection_chrome::TOOLBAR_GAP,
            toolbar_divider_color: selection_chrome::TOOLBAR_DIVIDER_COLOR,
            toolbar_divider_width: selection_chrome::TOOLBAR_DIVIDER_WIDTH,
            toolbar_elevation_offset_y: selection_chrome::TOOLBAR_ELEVATION_OFFSET_Y,
            toolbar_elevation_blur: selection_chrome::TOOLBAR_ELEVATION_BLUR,
            toolbar_elevation_spread: selection_chrome::TOOLBAR_ELEVATION_SPREAD,
            toolbar_shadow_color: selection_chrome::TOOLBAR_SHADOW_COLOR,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mirrors_the_authoritative_consts() {
        // 「Default は const を反映する」不変条件を固定し、この構造体を
        // 忘れた将来の const 編集をテスト時に検出する。
        let d = ChromeTuning::default();
        assert_eq!(d.scrollbar_thickness, scene_build::SCROLLBAR_THICKNESS);
        assert_eq!(d.scrollbar_thumb_color, scene_build::SCROLLBAR_THUMB_COLOR);
        assert_eq!(
            d.selection_highlight_color,
            scene_build::SELECTION_HIGHLIGHT_COLOR
        );
        assert_eq!(d.placeholder_alpha, scene_build::PLACEHOLDER_ALPHA);
        assert_eq!(
            d.composition_underline_thick,
            scene_build::COMPOSITION_UNDERLINE_THICK
        );
        assert_eq!(
            d.toolbar_corner_radius,
            selection_chrome::TOOLBAR_CORNER_RADIUS
        );
        assert_eq!(d.toolbar_height, selection_chrome::TOOLBAR_HEIGHT);
        assert_eq!(
            d.toolbar_label_font_size,
            selection_chrome::TOOLBAR_LABEL_FONT_SIZE
        );
        assert_eq!(
            d.toolbar_button_pad_x,
            selection_chrome::TOOLBAR_BUTTON_PAD_X
        );
        assert_eq!(d.toolbar_gap, selection_chrome::TOOLBAR_GAP);
        assert_eq!(
            d.toolbar_divider_color,
            selection_chrome::TOOLBAR_DIVIDER_COLOR
        );
        assert_eq!(
            d.toolbar_divider_width,
            selection_chrome::TOOLBAR_DIVIDER_WIDTH
        );
        assert_eq!(
            d.toolbar_elevation_offset_y,
            selection_chrome::TOOLBAR_ELEVATION_OFFSET_Y
        );
        assert_eq!(
            d.toolbar_elevation_blur,
            selection_chrome::TOOLBAR_ELEVATION_BLUR
        );
        assert_eq!(
            d.toolbar_elevation_spread,
            selection_chrome::TOOLBAR_ELEVATION_SPREAD
        );
        assert_eq!(
            d.toolbar_shadow_color,
            selection_chrome::TOOLBAR_SHADOW_COLOR
        );
    }
}
