//! Diagnose harness for issue #390 — Canvas text layout divergence vs DOM:
//! (1) short text in a text-input wraps to 1 char/line, (2) button text clips
//! at the top instead of centering, (3) the input border is a left-edge sliver.
//!
//! This reproduces the CSS-gallery `PopCard` demo container (a column flex with
//! `align-items: flex-start`) holding the actual gallery elements, then prints
//! the layout geometry so the root cause is a sharp, deterministic signal.
//!
//! Findings (env-gated; run with `HAYATE_DIAGNOSE=1 … -- --nocapture`):
//!   * The divergence is entirely in core layout (Taffy projection), upstream
//!     of scene rendering — so vello and tiny-skia reproduce it identically.
//!   * Root cause A (symptoms 1 & 3) — FIXED in issue #403: `text-input` was a
//!     Taffy leaf with no measure fn → 0 intrinsic content width, collapsing to
//!     padding-only width under width:auto + no flex-grow + a non-stretch
//!     cross-axis. It now carries the browser `<input>` UA default width (N=20
//!     chars in the resolved font); the assertion below guards the fix.
//!   * Root cause B (symptom 2): `button` projects to a plain Taffy flex box; it
//!     does not bake the browser `<button>` UA default of centering its content,
//!     so the label is stretched (align-items:stretch) and its glyphs paint at
//!     the box top. Independent of content width.

use hayate_core::{
    AlignValue, BorderStyleValue, Color, Dimension, DisplayValue, ElementId, ElementKind,
    ElementTree, FlexDirectionValue, StyleProp,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

fn input_style() -> Vec<StyleProp> {
    // Mirrors theme.ts `inputStyle()` — note: NO width, NO flex-grow.
    vec![
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::PaddingLeft(Dimension::px(12.0)),
        StyleProp::PaddingRight(Dimension::px(12.0)),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(Color::new(0.85, 0.83, 0.78, 1.0)),
        StyleProp::FontSize(13.0),
    ]
}

#[test]
fn diagnose_390() {
    if std::env::var_os("HAYATE_DIAGNOSE").is_none() {
        return;
    }
    let mut tree = ElementTree::new();
    tree.register_font("Inter", FONT.to_vec());
    let mut next = 1u64;
    let mut mk = |tree: &mut ElementTree, kind: ElementKind, styles: &[StyleProp]| {
        let id = tree.element_create(next, kind);
        next += 1;
        tree.element_set_style(id, styles);
        id
    };

    // Root surface
    let root = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::DefaultFontFamily("Inter".to_string()),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    tree.set_root(root);
    tree.set_viewport(300.0, 400.0);

    // PopCard demo container: column flex, align-items: flex-start, padding 14.
    let demo = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(8.0)),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Padding(Dimension::px(14.0)),
        ],
    );
    tree.element_append_child(root, demo);

    // (A) text-input with placeholder, inputStyle (no width).
    let input = mk(&mut tree, ElementKind::TextInput, &input_style());
    tree.element_set_text(input, "Type here");
    tree.element_append_child(demo, input);

    // (B) button "Click" — height 36, padding, but NO display/align (relies on
    // browser default centering).
    let button = mk(
        &mut tree,
        ElementKind::Button,
        &[
            StyleProp::Height(Dimension::px(36.0)),
            StyleProp::PaddingLeft(Dimension::px(14.0)),
            StyleProp::PaddingRight(Dimension::px(14.0)),
            StyleProp::BorderRadius(10.0),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    let label = mk(&mut tree, ElementKind::Text, &[]);
    tree.element_set_text(label, "Click");
    tree.element_append_child(button, label);
    tree.element_append_child(demo, button);

    let _ = tree.render(0.0);

    let rect = |t: &ElementTree, id: ElementId| t.element_layout_rect(id).unwrap_or((-1.0, -1.0, -1.0, -1.0));
    let (dx, _dy, dw, _dh) = rect(&tree, demo);
    let (ix, _iy, iw, ih) = rect(&tree, input);
    let (bx, by, bw, bh) = rect(&tree, button);
    let (lx, ly, lw, lh) = rect(&tree, label);

    eprintln!("[D390] demo box: x={dx} w={dw} (content width = {})", dw - 28.0);
    eprintln!("[D390] INPUT  box: x={ix} w={iw} h={ih}  (content width = {})", iw - 24.0 - 2.0);
    eprintln!("[D390] BUTTON box: x={bx} y={by} w={bw} h={bh}");
    eprintln!(
        "[D390] LABEL  box: x={lx} y={ly} w={lw} h={lh}  (top gap above text = {}, bottom gap = {})",
        ly - by,
        (by + bh) - (ly + lh)
    );
    eprintln!(
        "[D390] root cause A FIXED (issue #403): input content width = {} → placeholder fits 1 line; border wraps the whole field (no left-edge sliver)",
        (iw - 26.0).max(0.0),
        // iw retained in the assertion below.
    );
    eprintln!(
        "[D390] symptom2 (root cause B): label box stretched to button height ({}), glyphs top-aligned — top gap {} == bottom gap {} only when centered",
        lh,
        ly - by,
        (by + bh) - (ly + lh),
    );

    // Renderer independence is structural: layout is resolved here in core,
    // before any SceneGraph walk, so both Scene Renderers observe these rects.
    //
    // Root cause A (issue #403) is fixed: the width-unspecified text-input now
    // carries the font-relative UA default width, so its content width is well
    // above 0 (no 1-char/line wrap, no left-edge border sliver). The dedicated
    // regression test lives in `tests/text_input_default_width.rs`.
    assert!(
        iw - 26.0 > 50.0,
        "regression guard (#403): input must carry the UA default width (content width = {})",
        iw - 26.0,
    );
    // Root cause B (button vertical centering, #402) still reproduces here.
    assert!((ly - by) < 2.0, "repro guard: label is top-aligned (top gap={})", ly - by);
}
