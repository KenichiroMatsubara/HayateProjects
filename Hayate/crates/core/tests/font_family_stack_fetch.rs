//! カンマ区切りの CSS font-family は単一の名前ではなく*スタック*。Canvas Mode は
//! これを分割し、各エントリの generic キーワードを解決し、リスト中の既知の名前付き
//! ファミリを先回りで FetchFont する。"Inter, Segoe UI, sans-serif" 文字列全体に対し
//! 単一の FetchFont を出してはならない（どのアダプタも URL に解決できない）。
//!
//! WASM 相当のフォント文脈（system_fonts: false, Latin のみのデフォルト）で公開
//! ElementTree API を通し、名前付きエントリが不在で要求される実経路を駆動する。

use hayate_core::{Dimension, ElementId, ElementKind, ElementTree, Event, StyleProp};

/// WASM バンドルの代役となる Latin のみのフェイス。Latin をカバーし .notdef なしで
/// シェイプさせ、先回りの名前付きフォント取得を分離する。
fn latin_only_default() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

/// WASM 相当のツリー（システムフォントなし、Latin のみのデフォルト）。font_family
/// スタックでスタイルした Latin text を持つ Text 要素を 1 つ含む。
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

/// "Inter, sans-serif" は名前付きビルトイン "Inter" のみを要求する。generic の
/// sans-serif はバンドル済みデフォルトに解決され（登録済みなので要求されない）、
/// スタック文字列全体は決して出さない。
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

/// カンマ文字列全体を単一ファミリとして要求してはならず、その中の名前付き
/// ビルトイン（Inter）は要求しなければならない。
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

/// generic キーワードはエントリ単位で解決する。serif → Noto Serif、sans-serif は
/// バンドル済みデフォルトに解決され要求されない。値全体ではなくエントリごとに解決
/// されることを示す。
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
