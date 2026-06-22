//! [`ImeBridge`] シームの iOS ソフトキーボード側（ADR-0069 / ADR-0113）。
//!
//! このモジュールは core が
//! [`ElementTree::drive_ime`](hayate_core::ElementTree::drive_ime) で算出する
//! [`ImePresentation`] を*反映するだけ*。`Shown` はホストビューを first responder にして
//! ソフトキーボードを上げ、`Hidden` は解除する。プラットフォームのキーボード制御
//! （Swift 側で `becomeFirstResponder` / `resignFirstResponder` に写る FFI
//! `hayate_ios_set_keyboard_visible`）の呼び出しはここ一箇所に集約し、編集可能性ゲートが
//! フレームループへ漏れ戻らないようにする。ガードテスト `tests/ime_api_encapsulation.rs`
//! が、クレート内の他所からこの FFI を呼ばないことを保証する（Android の
//! `show_soft_input` / `hide_soft_input` ガードと同型）。

use hayate_core::{ImeBridge, ImePresentation};

extern "C" {
    /// Swift（`HayateView.swift` の `@_cdecl`）が実装するソフトキーボード制御。
    /// first responder の取得/解放に写る。
    fn hayate_ios_set_keyboard_visible(visible: bool);
}

/// core の IME 提示を iOS のソフトキーボードに反映する。
///
/// 永続的な `shown` フラグへの借用を保持し、`Shown` / `Hidden` が連続しても FFI を
/// 再発行しない。キーボードの切り替えは表示状態が遷移したときだけ行う。
pub struct IosImeBridge<'a> {
    shown: &'a mut bool,
}

impl<'a> IosImeBridge<'a> {
    pub fn new(shown: &'a mut bool) -> Self {
        Self { shown }
    }
}

impl ImeBridge for IosImeBridge<'_> {
    fn present(&mut self, presentation: ImePresentation) {
        let want_keyboard = matches!(presentation, ImePresentation::Shown { .. });
        if want_keyboard == *self.shown {
            return;
        }
        // SAFETY: Swift ホストが `@_cdecl("hayate_ios_set_keyboard_visible")` を提供し、
        // メインスレッド（CADisplayLink / UI イベント）からのみ呼ばれる。
        unsafe { hayate_ios_set_keyboard_visible(want_keyboard) };
        *self.shown = want_keyboard;
    }
}
