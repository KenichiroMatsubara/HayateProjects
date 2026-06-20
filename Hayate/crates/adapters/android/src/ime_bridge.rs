//! Android soft-keyboard side of the [`ImeBridge`] seam (ADR-0069, #392).
//!
//! This module only *reflects* the [`ImePresentation`] core computes in
//! [`ElementTree::drive_ime`](hayate_core::ElementTree::drive_ime): `Shown`
//! raises the GameTextInput soft keyboard, `Hidden` dismisses it. It is the
//! single place the platform `show_soft_input` / `hide_soft_input` calls live,
//! so the editability gate can never drift back into the frame loop the way it
//! did before #392 (when each adapter hand-rolled the decision and the fix
//! landed for Android only). The guard test `tests/ime_api_encapsulation.rs`
//! enforces that nothing else in the crate calls those platform APIs.

use android_activity::AndroidApp;
use hayate_core::{ImeBridge, ImePresentation};

/// Reflects core's IME presentation onto the GameTextInput soft keyboard.
///
/// Holds a borrow of the persistent `shown` flag so repeated `Shown` /`Hidden`
/// frames don't re-issue the platform call — the keyboard is toggled only on a
/// visibility transition.
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
