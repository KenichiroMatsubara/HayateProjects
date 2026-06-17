//! Issue #344: a comma-separated CSS `font-family` value is a *stack*, not a
//! single family name. Canvas Mode must split it, resolve each entry's generic
//! keyword, and proactively `FetchFont` the known named families in the list —
//! not emit one `FetchFont` for the whole `"Inter, Segoe UI, sans-serif"`
//! string (which no adapter can resolve to a URL).
//!
//! These tests drive the real proactive-fetch path through the public
//! `ElementTree` API in a WASM-like font context (`system_fonts: false`,
//! Latin-only default) so the named entry is absent and gets requested.

use hayate_core::{Dimension, ElementId, ElementKind, ElementTree, Event, StyleProp};

/// A Latin-only face standing in for a WASM bundle: covers Latin so the Latin
/// text shapes without `.notdef`, isolating the *proactive* named-font fetch.
fn latin_only_default() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

/// A WASM-like tree (no system fonts, Latin-only default) with one Text element
/// holding Latin `text` styled with the CSS `font_family` stack.
fn wasm_like_tree(text: &str, font_family: &str) -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    tree.test_set_wasm_like_fonts(latin_only_default());
    let view = tree.element_create(1, ElementKind::View);
    let label = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(view, label);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_text(label, text);
    tree.element_set_font_family(label, font_family);
    (tree, label)
}

fn fetch_font_families(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::FetchFont { family } => Some(family.clone()),
            _ => None,
        })
        .collect()
}

/// Tracer: `"Inter, sans-serif"` must request the named builtin `"Inter"`
/// alone. The generic `sans-serif` resolves to the bundled default (already
/// registered, so not requested), and the full stack string is never emitted.
#[test]
fn multi_name_stack_requests_named_family_not_full_string() {
    let (mut tree, _label) = wasm_like_tree("Hello", "Inter, sans-serif");

    tree.render(0.0);
    let requested = fetch_font_families(&tree.poll_events());

    assert_eq!(
        requested,
        vec!["Inter".to_string()],
        "a font stack must fetch the named family alone, never the full list string"
    );
}

/// The issue's own example: the full comma string must never be requested as a
/// single family, and the named builtin in it (`Inter`) must be requested.
#[test]
fn full_stack_string_is_never_requested_as_one_family() {
    let stack = "Inter, Segoe UI, system-ui, sans-serif";
    let (mut tree, _label) = wasm_like_tree("Hello", stack);

    tree.render(0.0);
    let requested = fetch_font_families(&tree.poll_events());

    assert!(
        requested.iter().any(|f| f == "Inter"),
        "the named builtin in the stack must be requested: {requested:?}"
    );
    assert!(
        !requested.iter().any(|f| f == stack),
        "the whole comma string must never be requested as one family: {requested:?}"
    );
}

/// Generic keywords resolve per entry: `serif` → `Noto Serif`, while the
/// `sans-serif` entry resolves to the already-bundled default and is not
/// requested. Proves resolution happens entry-by-entry, not on the whole value.
#[test]
fn generic_keywords_resolve_per_entry() {
    let (mut tree, _label) = wasm_like_tree("Hello", "serif, sans-serif");

    tree.render(0.0);
    let requested = fetch_font_families(&tree.poll_events());

    assert_eq!(
        requested,
        vec!["Noto Serif".to_string()],
        "`serif` must resolve to Noto Serif; the default-bound `sans-serif` is not refetched"
    );
}
