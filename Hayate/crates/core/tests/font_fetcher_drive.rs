//! `ElementTree::drive_font_requests` はオンデマンドフォント取得のシーム（ADR-0132
//! スライス2）。`drive_ime` と同型で、欠落フォント検出のたびに `FontFetcher::request`
//! を同期に呼ぶ。既存の `FetchFont` イベント経路（`font_fetch_retry.rs`）を置き換える
//! ものではなく、アダプタがそれを手動 poll する代わりに使う高レベルの入口。

use hayate_core::{Dimension, ElementId, ElementKind, ElementTree, FontFetcher, StyleProp};

/// core が呼んだリクエストを記録するだけの `FontFetcher`。
#[derive(Default)]
struct FakeFontFetcher {
    requested: Vec<String>,
}

impl FontFetcher for FakeFontFetcher {
    fn request(&mut self, family: &str) {
        self.requested.push(family.to_string());
    }
}

/// WASM 相当のバンドル代役（CI 常設の DejaVu Sans、Latin のみ）。
fn latin_only_default() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

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
    tree.element_set_font_family(label, "Noto Sans JP");
    (tree, label)
}

#[test]
fn missing_font_drives_exactly_one_request() {
    let (mut tree, _label) = wasm_like_tree_with_text("あ");
    tree.render(0.0);

    let mut fetcher = FakeFontFetcher::default();
    tree.drive_font_requests(&mut fetcher);

    assert_eq!(
        fetcher.requested,
        vec!["Noto Sans JP".to_string()],
        "a missing CJK family must drive exactly one FontFetcher::request"
    );
}

#[test]
fn a_tree_with_no_missing_fonts_drives_no_requests() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.render(0.0);

    let mut fetcher = FakeFontFetcher::default();
    tree.drive_font_requests(&mut fetcher);

    assert!(
        fetcher.requested.is_empty(),
        "no missing fonts must not drive any request: {:?}",
        fetcher.requested
    );
}

#[test]
fn in_flight_family_is_not_re_driven_on_the_next_frame() {
    let (mut tree, _label) = wasm_like_tree_with_text("あ");
    tree.render(0.0);

    let mut first = FakeFontFetcher::default();
    tree.drive_font_requests(&mut first);
    assert_eq!(first.requested, vec!["Noto Sans JP".to_string()]);

    tree.render(16.0);
    let mut second = FakeFontFetcher::default();
    tree.drive_font_requests(&mut second);
    assert!(
        second.requested.is_empty(),
        "a family already in flight must not be re-requested: {:?}",
        second.requested
    );
}
