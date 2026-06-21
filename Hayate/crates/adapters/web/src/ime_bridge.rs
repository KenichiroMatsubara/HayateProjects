use hayate_core::{CharacterBounds, ImeBridge, ImePresentation};

/// Web EditContext ブリッジ（ADR-0069）。core が毎フレーム計算する
/// [`ImePresentation`] を反映するだけ。`text-input` がフォーカス中か（`visible`）と
/// キャレット位置（`last_bounds`）を保持する。JS ホストは `visible` を見て
/// `EditContext` を着脱し（これがモバイルのソフトキーボードを表示/解除する）、
/// `last_bounds` を `updateControlBounds` / `updateSelectionBounds` で候補ウィンドウ
/// 配置に使う。編集可否の判断はアダプタ側では行わず、`ElementTree::drive_ime` が持つ。
#[derive(Clone, Copy, Debug, Default)]
pub struct WebImeBridge {
    last_bounds: CharacterBounds,
    visible: bool,
}

impl WebImeBridge {
    pub fn last_bounds(&self) -> CharacterBounds {
        self.last_bounds
    }

    /// 今フレームで core がソフトキーボードを上げたいか（`text-input` がフォーカス中か）。
    /// JS ホストはこれが true の間だけ `EditContext` を着ける。
    pub fn visible(&self) -> bool {
        self.visible
    }
}

impl ImeBridge for WebImeBridge {
    fn present(&mut self, presentation: ImePresentation) {
        match presentation {
            ImePresentation::Hidden => self.visible = false,
            ImePresentation::Shown { bounds } => {
                self.visible = true;
                self.last_bounds = bounds;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{Dimension, ElementKind, ElementTree, StyleProp};

    #[test]
    fn focused_text_input_makes_the_bridge_visible_with_bounds() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
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

        let mut bridge = WebImeBridge::default();
        tree.drive_ime(&mut bridge);
        assert!(bridge.visible(), "text-input focus must arm the keyboard");
        let bounds = bridge.last_bounds();
        assert!(bounds.width > 0.0);
        assert!(bounds.height > 0.0);
    }

    #[test]
    fn focusing_a_non_input_keeps_the_bridge_hidden() {
        let mut tree = ElementTree::new();
        let view = tree.element_create(1, ElementKind::View);
        let text = tree.element_create(2, ElementKind::Text);
        tree.element_append_child(view, text);
        tree.set_root(view);
        tree.set_viewport(200.0, 40.0);
        tree.element_focus(text);
        tree.render(0.0);

        let mut bridge = WebImeBridge::default();
        tree.drive_ime(&mut bridge);
        assert!(
            !bridge.visible(),
            "focusing plain text must not arm the soft keyboard (#392)"
        );
    }
}
