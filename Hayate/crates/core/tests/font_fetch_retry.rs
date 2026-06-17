//! Issue #343: an on-demand font fetch that fails must not latch the family in
//! `pending_font_fetches` forever. The family has to stay eligible for a later
//! re-request, with a finite retry budget so a permanently-failing family never
//! runs away.
//!
//! These tests pin the WASM environment (`system_fonts: false`, ADR-0042) with a
//! Latin-only default font so Japanese text shapes to `.notdef` and drives the
//! real `FetchFont → register_font` path through the public `ElementTree` API.

use hayate_core::{Dimension, ElementId, ElementKind, ElementTree, Event, StyleProp};

static NOTO_SANS_JP: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// A Latin-only face standing in for a WASM bundle that does not cover CJK.
/// DejaVu Sans is present on the CI image and covers Latin but not Japanese.
fn latin_only_default() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

/// A WASM-like tree (no system fonts, Latin-only default) with one Text element
/// holding `text`. Returns the tree and the text element id.
fn wasm_like_tree_with_text(text: &str) -> (ElementTree, ElementId) {
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
    // Name the CJK family explicitly: it is absent from the WASM-like collection,
    // so it is requested on demand and, once registered, shapes the text directly.
    tree.element_set_font_family(label, "Noto Sans JP");
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

/// Tracer: Japanese text on a Latin-only bundle shapes to `.notdef`, so the
/// missing family is requested once — and not re-requested while in flight.
#[test]
fn missing_family_is_requested_once_then_quiet_while_in_flight() {
    let (mut tree, _label) = wasm_like_tree_with_text("あ");

    tree.render(0.0);
    let first = fetch_font_families(&tree.poll_events());
    assert_eq!(
        first,
        vec!["Noto Sans JP".to_string()],
        "first frame must request the absent CJK family exactly once"
    );

    // Still in flight (no success, no failure reported): a re-render must not
    // re-request the same family.
    tree.render(16.0);
    let second = fetch_font_families(&tree.poll_events());
    assert!(
        second.is_empty(),
        "family already in flight must not be re-requested: {second:?}"
    );
}

/// The core fix: a reported fetch failure must NOT leave the family latched in
/// `pending`. A later frame re-requests it (the adapter's transient-failure
/// path: 403/429/blip on a fresh deploy must be retried).
#[test]
fn failed_fetch_is_requested_again_on_a_later_frame() {
    let (mut tree, _label) = wasm_like_tree_with_text("あ");

    tree.render(0.0);
    let first = fetch_font_families(&tree.poll_events());
    assert_eq!(first, vec!["Noto Sans JP".to_string()]);

    // The adapter's fetch failed (transient CDN error). It reports the failure
    // back to core; core returns `true` because the retry budget is not spent.
    assert!(
        tree.font_fetch_failed("Noto Sans JP"),
        "a first failure must be retryable, not terminal"
    );

    tree.render(16.0);
    let retry = fetch_font_families(&tree.poll_events());
    assert_eq!(
        retry,
        vec!["Noto Sans JP".to_string()],
        "a failed family must be re-requested on a later frame, not latched"
    );
}

/// A family that keeps failing must be given up on after a finite budget, so
/// neither re-requests nor logs run away during a sustained CDN outage.
#[test]
fn permanently_failing_family_is_given_up_and_not_re_requested() {
    let (mut tree, label) = wasm_like_tree_with_text("あ");

    // Drive request → fail cycles until core reports it has given up.
    let mut gave_up = false;
    for frame in 0..10 {
        tree.render(frame as f64 * 16.0);
        let requested = fetch_font_families(&tree.poll_events());
        if requested.is_empty() {
            // Nothing requested this frame: core has stopped asking.
            break;
        }
        assert_eq!(requested, vec!["Noto Sans JP".to_string()]);
        if !tree.font_fetch_failed("Noto Sans JP") {
            gave_up = true;
            break;
        }
    }
    assert!(gave_up, "a family that always fails must eventually be given up on");

    // After give-up, even forcing a re-shape must not re-request the family.
    tree.element_set_text(label, "あい");
    tree.render(1_000.0);
    let after = fetch_font_families(&tree.poll_events());
    assert!(
        after.is_empty(),
        "a given-up family must never be requested again: {after:?}"
    );
}

/// Acceptance (issue #343): first fetch fails, the family is re-requested on a
/// later frame, the retry "succeeds" (the font is registered), and the CJK text
/// then shapes to real glyphs instead of `.notdef` tofu.
#[test]
fn first_fetch_fails_then_retry_succeeds_and_glyphs_render() {
    let (mut tree, label) = wasm_like_tree_with_text("あ");

    // Frame 1: the absent family is requested; the fetch fails (transient).
    tree.render(0.0);
    assert_eq!(
        fetch_font_families(&tree.poll_events()),
        vec!["Noto Sans JP".to_string()]
    );
    assert!(
        tree.test_element_glyph_ids(label).iter().any(|&id| id == 0),
        "before the font loads the CJK glyph must be .notdef (tofu)"
    );
    assert!(tree.font_fetch_failed("Noto Sans JP"));

    // Frame 2: re-requested after the failure; this time the fetch succeeds and
    // the adapter registers the face.
    tree.render(16.0);
    assert_eq!(
        fetch_font_families(&tree.poll_events()),
        vec!["Noto Sans JP".to_string()],
        "the family must be re-requested after a transient failure"
    );
    tree.register_font("Noto Sans JP", NOTO_SANS_JP.to_vec());

    // Frame 3: re-shaped with the real font — every glyph is a real id, no tofu,
    // and nothing is requested anymore.
    tree.render(32.0);
    assert!(
        fetch_font_families(&tree.poll_events()).is_empty(),
        "once loaded, the family must not be requested again"
    );
    let glyphs = tree.test_element_glyph_ids(label);
    assert!(!glyphs.is_empty(), "text must have shaped to glyphs");
    assert!(
        glyphs.iter().all(|&id| id != 0),
        "after the retry succeeds the CJK text must render real glyphs, not .notdef: {glyphs:?}"
    );
}
