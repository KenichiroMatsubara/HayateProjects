//! [`ImeBridge`] シームの Android ソフトキーボード側（ADR-0069）。
//!
//! このモジュールは core が
//! [`ElementTree::drive_ime`](hayate_core::ElementTree::drive_ime) で算出する
//! [`ImePresentation`] を*反映するだけ*。`Shown` は GameTextInput のソフト
//! キーボードを上げ、`Hidden` は閉じる。プラットフォームの `show_soft_input` /
//! `hide_soft_input` 呼び出しはここ一箇所に集約し、編集可能性ゲートがフレーム
//! ループへ漏れ戻らないようにする。ガードテスト
//! `tests/ime_api_encapsulation.rs` が、クレート内の他所からこれらのプラット
//! フォーム API を呼ばないことを保証する。

use android_activity::AndroidApp;
use hayate_core::{ImeBridge, ImePresentation};

/// core の IME 提示を GameTextInput のソフトキーボードに反映する。
///
/// 永続的な `shown` フラグへの借用を保持し、`Shown` / `Hidden` が連続しても
/// プラットフォーム呼び出しを再発行しない。キーボードの切り替えは表示状態が
/// 遷移したときだけ行う。
pub struct AndroidImeBridge<'a> {
    app: &'a AndroidApp,
    shown: &'a mut bool,
}

impl<'a> AndroidImeBridge<'a> {
    pub fn new(app: &'a AndroidApp, shown: &'a mut bool) -> Self {
        Self { app, shown }
    }
}

impl ImeBridge for AndroidImeBridge<'_> {
    fn present(&mut self, presentation: ImePresentation) {
        let want_keyboard = matches!(presentation, ImePresentation::Shown { .. });
        if want_keyboard == *self.shown {
            return;
        }
        if want_keyboard {
            self.app.show_soft_input(true);
        } else {
            self.app.hide_soft_input(true);
        }
        *self.shown = want_keyboard;
    }
}
