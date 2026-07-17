//! ソフトキーボードの表示可否は `ElementTree::drive_ime` が一元的に決める
//! （ADR-0069）。各アダプタはそれが出す [`ImePresentation`] を反映するだけなので、
//! 本テストは全プラットフォームの編集可否ゲートをまとめて固定する。素のタップは
//! 当たった要素にフォーカスする（Chromium 互換、ADR-0102）が、キーボードを上げて
//! よいのはフォーカス中の `text-input` のときだけ。

use hayate_core::{Dimension, ElementKind, ElementTree, ImeBridge, ImePresentation, StyleProp};

/// core が最後に要求した presentation を記録する。
#[derive(Default)]
struct FakeIme {
    last: Option<ImePresentation>,
}

impl ImeBridge for FakeIme {
    fn present(&mut self, presentation: ImePresentation) {
        self.last = Some(presentation);
    }
}

fn styled(tree: &mut ElementTree, id: u64, kind: ElementKind) -> hayate_core::ElementId {
    let el = tree.element_create(id, kind);
    tree.element_set_style(
        el,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    el
}

#[test]
fn nothing_focused_keeps_the_keyboard_hidden() {
    let mut tree = ElementTree::new();
    let root = styled(&mut tree, 1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 40.0);
    tree.render(0.0);

    let mut ime = FakeIme::default();
    tree.drive_ime(&mut ime);
    assert_eq!(ime.last, Some(ImePresentation::Hidden));
}

#[test]
fn tapping_plain_text_does_not_raise_the_keyboard() {
    // 素のタップは当たった要素にフォーカスするが、非編集要素ではキーボードを
    // 下げたままにする（全タップで上がってしまう不具合への対処）。
    let mut tree = ElementTree::new();
    let root = styled(&mut tree, 1, ElementKind::View);
    let text = styled(&mut tree, 2, ElementKind::Text);
    tree.element_append_child(root, text);
    tree.set_root(root);
    tree.set_viewport(200.0, 40.0);
    tree.element_focus(text);
    tree.render(0.0);

    let mut ime = FakeIme::default();
    tree.drive_ime(&mut ime);
    assert_eq!(
        ime.last,
        Some(ImePresentation::Hidden),
        "focusing plain text must not arm the soft keyboard"
    );
}

#[test]
fn focusing_a_text_input_shows_the_keyboard() {
    let mut tree = ElementTree::new();
    let root = styled(&mut tree, 1, ElementKind::View);
    let input = styled(&mut tree, 2, ElementKind::TextInput);
    tree.element_append_child(root, input);
    tree.set_root(root);
    tree.set_viewport(200.0, 40.0);
    tree.element_append_text_content(input, "hi");
    tree.element_focus(input);
    tree.render(0.0);

    let mut ime = FakeIme::default();
    tree.drive_ime(&mut ime);
    match ime.last {
        Some(ImePresentation::Shown { bounds }) => {
            assert!(bounds.width >= 0.0 && bounds.height >= 0.0);
        }
        other => panic!("text-input focus must show the keyboard, got {other:?}"),
    }
}
