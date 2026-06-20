//! `ElementTree::drive_ime` is the single place soft-keyboard visibility is
//! decided (ADR-0069, #392). Every adapter reflects the [`ImePresentation`] it
//! emits, so these tests lock the editability gate for *all* platforms at once:
//! a plain tap (which focuses whatever it hits — Chromium parity, ADR-0102) must
//! not raise the keyboard, only a focused `text-input` does. Before this, the
//! gate was hand-rolled per adapter and the fix landed for Android only (#392).

use hayate_core::{
    Dimension, ElementKind, ElementTree, ImeBridge, ImePresentation, StyleProp,
};

/// Records the last presentation core asked for.
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
    // A plain tap focuses whatever it hits, but a non-editable element must keep
    // the keyboard down — the mobile bug was every tap raising it (#392).
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
