//! draw テキストのフォント供給ライフサイクル（#732）。
//!
//! 要素テキストは既に方針を持っている——**止めない・必ず何か描く・後のフレームで
//! 直す**。draw テキストはこの輪に「そのまま乗る」ことになっており、本テストが
//! その配線を押さえる:
//!
//! 1. 欠落コードポイントを含む draw テキストが `Event::FetchFont` を積む
//!    （draw を載せるのは `view` で `is_text_like()` ではないので、
//!    `collect_missing_into` の対象から外れると**発火しない**）
//! 2. フォントが届いた後（`fonts_dirty`）に draw テキストが**再シェープされる**
//!    （`is_text_like()` で絞ると draw が取り残され、豆腐のまま二度と直らない）
//!
//! どちらも例外を出さず絵だけが壊れる種類の破れなので、外形（発火したイベントと
//! 記録された painter op）で主張する。

use hayate_core::{
    render_scene_graph, DrawCommand, DrawOp, DrawPaint, ElementKind, ElementTree, Event,
    RecordingPainter, TextRunId,
};

/// WASM バンドルの代役となる Latin のみのフェイス。CJK を持たないので、日本語の
/// draw テキストは .notdef を生み欠落 family の検出経路が走る。
fn latin_only_default() -> Vec<u8> {
    std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .expect("DejaVuSans.ttf present for the test")
}

/// テストが使う名前付きファミリ。最初は collection 未登録なので、これを指した
/// draw テキストは `FetchFont` を積みつつ既定へフォールバックして描かれる。
const NAMED_FAMILY: &str = "Noto Sans JP";

fn draw_text(text: &str) -> Vec<DrawCommand> {
    vec![DrawCommand::Text {
        text: text.to_string(),
        x: 0.0,
        y: 0.0,
        font_size: 24.0,
        font_weight: 400.0,
        font_style: 0.0,
        font_family: NAMED_FAMILY.to_string(),
        paint: DrawPaint {
            color: [0.0, 0.0, 0.0, 1.0],
            ..Default::default()
        },
    }]
}

/// draw を 1 つ載せた `view` だけの WASM 相当ツリー。
fn tree_with_draw_text(text: &str) -> ElementTree {
    let mut tree = ElementTree::new();
    tree.test_set_wasm_like_fonts(latin_only_default());
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_draw(root, draw_text(text));
    tree
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

/// painter が実際に塗ったグリフランの id 列。再シェープの観測点にする
/// （`DrawOp` は `PartialEq` を持たないので id で比べる）。
fn painted_text_runs(tree: &ElementTree) -> Vec<TextRunId> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter
        .into_ops()
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::DrawTextRun { text_run, .. } => Some(text_run),
            _ => None,
        })
        .collect()
}

// draw テキストの欠落グリフが `FetchFont` を積む。
//
// これは要素テキストとまったく同じ供給経路で、draw だけが外れていると
// 「日本語が永久に豆腐」になる。
#[test]
fn missing_glyphs_in_draw_text_request_a_font() {
    let mut tree = tree_with_draw_text("将棋");
    tree.render(0.0);

    let requested = fetch_font_families(&tree.poll_events());
    assert!(
        requested.iter().any(|f| f == NAMED_FAMILY),
        "draw text must request its missing named family, got {requested:?}"
    );
}

// フォントを止めない: グリフが足りなくても draw テキストは**必ず何か描く**
// （豆腐でよい）。フレームを落としたり描画を先送りしたりしない。
#[test]
fn draw_text_paints_even_before_its_font_arrives() {
    let mut tree = tree_with_draw_text("将棋");
    tree.render(0.0);

    assert!(
        !painted_text_runs(&tree).is_empty(),
        "draw text must paint something on the first frame, even with missing glyphs"
    );
}

// フォントが届いたら draw テキストが再シェープされる。
//
// `fonts_dirty` の再シェープ対象を `is_text_like()` だけで絞ると、draw を運ぶ
// `view` が漏れてここが破れる（届いたフォントが反映されず豆腐で固定される）。
// 「再シェープされたこと」は `TextRunId` が別物に変わったことで観測する。
#[test]
fn draw_text_is_reshaped_once_the_font_arrives() {
    let mut tree = tree_with_draw_text("将棋");
    tree.render(0.0);
    let before = painted_text_runs(&tree);
    assert!(!before.is_empty(), "baseline paint");

    // 実際に CJK を持つフォントが届いたことにする。バンドル既定に組み込まれ、
    // `mark_fonts_dirty` が全テキストの再シェープを要求する。
    // バンドル済みの実フォント（`include_bytes!` で core に載っているのと同じ face）。
    // 日本語グリフに CDN は要らない — 実行時取得が要るのは韓国語・簡繁体・記号・絵文字。
    let cjk = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/fonts/NotoSansJP.ttf"
    ))
    .expect("bundled NotoSansJP.ttf");
    tree.register_font(NAMED_FAMILY, cjk);
    tree.render(16.0);

    let after = painted_text_runs(&tree);
    assert!(!after.is_empty(), "draw text must still paint after reshape");
    assert_ne!(
        before, after,
        "draw text must be reshaped when a font arrives; identical text runs mean \
         fonts_dirty skipped the draw-carrying element and the glyphs are frozen as tofu"
    );
}
