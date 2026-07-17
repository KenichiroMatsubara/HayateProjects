//! オンデマンドのフォント取得が失敗しても、ファミリを `pending_font_fetches` に
//! 永久にラッチしてはならない。再リクエスト可能な状態を保ちつつ、有限のリトライ
//! 予算で恒久失敗のファミリが暴走しないようにする。
//!
//! これらのテストは WASM 環境（`system_fonts: false`, ADR-0042）を Latin のみの
//! デフォルトフォントで固定し、日本語テキストを `.notdef` にシェイプさせて、
//! 公開 `ElementTree` API 経由で実際の `FetchFont → register_font` 経路を駆動する。

use hayate_core::{Dimension, ElementId, ElementKind, ElementTree, Event, StyleProp};

static NOTO_SANS_JP: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

/// CJK を含まない WASM バンドルの代役となる Latin のみのフェイス。DejaVu Sans は
/// CI イメージに存在し、Latin はカバーするが日本語はカバーしない。
fn latin_only_default() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

/// WASM 相当のツリー（システムフォントなし、Latin のみのデフォルト）に、`text`
/// を持つ Text 要素を 1 つ置く。ツリーとその text 要素 id を返す。
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
    // CJK ファミリを明示指定する。WASM 相当のコレクションには無いためオンデマンドで
    // リクエストされ、登録されればテキストを直接シェイプする。
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

/// Latin のみのバンドル上の日本語テキストは `.notdef` にシェイプされ、欠落
/// ファミリは一度だけリクエストされ、取得中は再リクエストされない。
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

    // 取得中（成功も失敗も未報告）は、再描画で同じファミリを再リクエストしない。
    tree.render(16.0);
    let second = fetch_font_families(&tree.poll_events());
    assert!(
        second.is_empty(),
        "family already in flight must not be re-requested: {second:?}"
    );
}

/// 取得失敗の報告でファミリを `pending` にラッチしたままにしてはならない。後続
/// フレームで再リクエストする（一時失敗経路。デプロイ直後の 403/429/瞬断は
/// リトライ対象）。
#[test]
fn failed_fetch_is_requested_again_on_a_later_frame() {
    let (mut tree, _label) = wasm_like_tree_with_text("あ");

    tree.render(0.0);
    let first = fetch_font_families(&tree.poll_events());
    assert_eq!(first, vec!["Noto Sans JP".to_string()]);

    // 取得が失敗（CDN の一時エラー）。core へ失敗を報告すると、リトライ予算が
    // 残っているため core は `true` を返す。
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

/// 失敗し続けるファミリは有限の予算で諦める。CDN 障害が続いても再リクエストや
/// ログが暴走しない。
#[test]
fn permanently_failing_family_is_given_up_and_not_re_requested() {
    let (mut tree, label) = wasm_like_tree_with_text("あ");

    // core が諦めを報告するまで「リクエスト → 失敗」を繰り返す。
    let mut gave_up = false;
    for frame in 0..10 {
        tree.render(frame as f64 * 16.0);
        let requested = fetch_font_families(&tree.poll_events());
        if requested.is_empty() {
            // このフレームでリクエストなし。core が要求を止めた。
            break;
        }
        assert_eq!(requested, vec!["Noto Sans JP".to_string()]);
        if !tree.font_fetch_failed("Noto Sans JP") {
            gave_up = true;
            break;
        }
    }
    assert!(
        gave_up,
        "a family that always fails must eventually be given up on"
    );

    // 諦めた後は、再シェイプを強制してもファミリを再リクエストしない。
    tree.element_set_text(label, "あい");
    tree.render(1_000.0);
    let after = fetch_font_families(&tree.poll_events());
    assert!(
        after.is_empty(),
        "a given-up family must never be requested again: {after:?}"
    );
}

/// 受け入れ: 初回取得が失敗 → 後続フレームで再リクエスト → リトライが「成功」
/// （フォント登録）し、CJK テキストが `.notdef` 豆腐ではなく実グリフにシェイプ
/// される。
#[test]
fn first_fetch_fails_then_retry_succeeds_and_glyphs_render() {
    let (mut tree, label) = wasm_like_tree_with_text("あ");

    // フレーム 1: 欠落ファミリをリクエストし、取得が失敗（一時的）。
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

    // フレーム 2: 失敗後に再リクエスト。今回は取得成功でフェイスを登録する。
    tree.render(16.0);
    assert_eq!(
        fetch_font_families(&tree.poll_events()),
        vec!["Noto Sans JP".to_string()],
        "the family must be re-requested after a transient failure"
    );
    tree.register_font("Noto Sans JP", NOTO_SANS_JP.to_vec());

    // フレーム 3: 実フォントで再シェイプ。全グリフが実 id で豆腐なし、以降の
    // リクエストもない。
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
