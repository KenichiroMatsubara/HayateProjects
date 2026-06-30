//! Material ドラッグハンドル + 長押し選択というモバイル流の選択 chrome（ADR-0097）。
//! ハンドルとそのジオメトリ、ハンドルドラッグによる端点調整、長押し単語選択を
//! 公開 `ElementTree` 経由で検証する。

use hayate_core::{
    Dimension, DrawOp, ElementId, ElementKind, ElementTree, PointerKind, RecordingPainter,
    SelectionHandleEnd, StyleProp, render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// `<view [selectable]><text "Hello world"></view>` を 1 行で組み、
/// (tree, view, text) を返す。`selection_toolbar.rs` のハーネスと同型。
fn selectable_paragraph() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, text);
    tree.element_set_text(text, "Hello world");
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, view, text)
}

/// Touch で先頭範囲をドラッグ選択して離す。Touch モダリティで非空の選択が残るため、
/// ドラッグハンドルが立ち上がる（ADR-0104）。
fn select_a_range(tree: &mut ElementTree) {
    tree.on_pointer_down_with_kind(2.0, 8.0, 0, PointerKind::Touch);
    tree.on_pointer_move(70.0, 8.0);
    tree.on_pointer_up(70.0, 8.0);
}

#[test]
fn selection_raises_a_handle_at_each_end() {
    let (mut tree, _view, _text) = selectable_paragraph();
    select_a_range(&mut tree);

    let handles = tree
        .selection_handles()
        .expect("a non-empty selection raises drag handles");
    assert_eq!(handles.start.end, SelectionHandleEnd::Start);
    assert_eq!(handles.end.end, SelectionHandleEnd::End);
    // 左→右の範囲では start ハンドルが end ハンドルより左に来る。
    assert!(
        handles.start.knob_x < handles.end.knob_x,
        "start handle is left of the end handle",
    );
    // どちらのノブも 1 行のテキストの下にぶら下がる。
    assert!(handles.start.knob_y > 0.0);
}

#[test]
fn no_handles_without_a_selection() {
    let (tree, _view, _text) = selectable_paragraph();
    assert!(
        tree.selection_handles().is_none(),
        "no selection means no handles",
    );
}

#[test]
fn chrome_style_switch_recolors_the_handles_and_is_additive() {
    use hayate_core::SelectionChromeStyle;

    let knob_color = |style: SelectionChromeStyle| -> [f32; 4] {
        let (mut tree, _v, _t) = selectable_paragraph();
        tree.set_selection_chrome_style(style);
        select_a_range(&mut tree);
        tree.render(0.0);
        let h = tree.selection_handles().expect("handles");
        // しずく本体は隅別クリップで切り出すベタ塗り矩形（corner_radius=0）。
        draw_ops(&tree)
            .into_iter()
            .find_map(|op| match op {
                DrawOp::FillRect { x, y, width, height, corner_radius, color }
                    if (x + width / 2.0 - h.start.knob_x).abs() < 0.5
                        && (y + height / 2.0 - h.start.knob_y).abs() < 0.5
                        && (width - 2.0 * h.start.radius).abs() < 0.5
                        && corner_radius == 0.0 =>
                {
                    Some(color)
                }
                _ => None,
            })
            .expect("the start handle bulb rect")
    };

    // Material が既定。Cupertino への切り替えは加法的（同じハンドルモデルで
    // テーマだけ違う）で、ノブの色が変わる。
    assert_eq!(SelectionChromeStyle::default(), SelectionChromeStyle::Material);
    assert_ne!(
        knob_color(SelectionChromeStyle::Material),
        knob_color(SelectionChromeStyle::Cupertino),
        "the chrome style enum drives a visibly different handle",
    );
}

/// 本体中心 `(kx, ky)`・半径 `r` のしずく本体（隅別クリップで切り出すベタ塗りの
/// `2r` 四方 FillRect、`corner_radius` は 0）が描かれているか。色は問わない。
fn knob_drawn_at(ops: &[DrawOp], kx: f32, ky: f32, r: f32) -> bool {
    ops.iter().any(|op| {
        matches!(op,
            DrawOp::FillRect { x, y, width, height, corner_radius, .. }
                if (x + width / 2.0 - kx).abs() < 0.5
                    && (y + height / 2.0 - ky).abs() < 0.5
                    && (width - 2.0 * r).abs() < 0.5
                    && (height - 2.0 * r).abs() < 0.5
                    && *corner_radius == 0.0)
    })
}

/// ハンドル領域（テキスト行より下）の角丸クリップ矩形を `(corner_radii, box)` で
/// 集める。core はしずく型ハンドルを `PushClipRect{corner_radii} > FillRect` で描く。
fn handle_clips(ops: &[DrawOp]) -> Vec<([f32; 4], (f32, f32, f32, f32))> {
    ops.iter()
        .filter_map(|op| match op {
            DrawOp::PushClipRect { x, y, width, height, corner_radii }
                if *y > 15.0 && *width < 40.0 =>
            {
                Some((*corner_radii, (*x, *y, *width, *height)))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn handles_are_mirrored_teardrops_framing_the_selection() {
    // Chrome Android お手本: ドラッグハンドルは真円ではなく左右ミラーの「しずく型」。
    // start（左端）は tip が右上で本体が左下へ、end（右端）は tip が左上で本体が右下へ。
    // tip（角の隅 = 角丸 0）は選択の各端に触れ、本体は選択文字に被らず外側へぶら下がる。
    let (mut tree, _v, _t) = selectable_paragraph();
    select_a_range(&mut tree);
    tree.render(0.0);

    let h = tree.selection_handles().expect("handles");
    let r = h.start.radius;
    let clips = handle_clips(&draw_ops(&tree));
    assert_eq!(clips.len(), 2, "exactly two teardrop handle clips are drawn");

    // start = tip 右上（TR=0、他は r）、end = tip 左上（TL=0、他は r）の左右ミラー。
    let start = clips
        .iter()
        .find(|(radii, _)| *radii == [r, 0.0, r, r])
        .expect("start handle is a teardrop with a square top-right tip");
    let end = clips
        .iter()
        .find(|(radii, _)| *radii == [0.0, r, r, r])
        .expect("end handle is a teardrop with a square top-left tip");

    // 本体は選択を外から囲む: start の右端（tip）が左、end の左端（tip）が右に来て、
    // 二つの本体は重ならない（円のように選択文字へ食い込まない）。
    let start_right = start.1 .0 + start.1 .2;
    let end_left = end.1 .0;
    assert!(
        start_right <= end_left + 0.5,
        "the start bulb hangs left of the end bulb, framing the selection",
    );
}

#[test]
fn handles_are_drawn_by_core_during_selection() {
    let (mut tree, _v, _t) = selectable_paragraph();
    select_a_range(&mut tree);
    tree.render(0.0);

    let handles = tree.selection_handles().expect("handles after selecting");
    let ops = draw_ops(&tree);
    assert!(
        knob_drawn_at(&ops, handles.start.knob_x, handles.start.knob_y, handles.start.radius),
        "the start handle bulb is drawn at its position",
    );
    assert!(
        knob_drawn_at(&ops, handles.end.knob_x, handles.end.knob_y, handles.end.radius),
        "the end handle bulb is drawn at its position",
    );
}

#[test]
fn handles_disappear_from_the_scene_when_the_selection_clears() {
    let (mut tree, _v, _t) = selectable_paragraph();
    select_a_range(&mut tree);
    tree.render(0.0);
    let handles = tree.selection_handles().expect("handles");
    let (sx, sy, sr) = (handles.start.knob_x, handles.start.knob_y, handles.start.radius);
    assert!(knob_drawn_at(&draw_ops(&tree), sx, sy, sr), "knob present while selecting");

    // 空白部分をタップして選択を解除し、再描画する。
    tree.on_pointer_down(2.0, 150.0);
    tree.on_pointer_up(2.0, 150.0);
    tree.render(0.0);

    assert!(tree.selection_handles().is_none(), "selection cleared");
    assert!(
        !knob_drawn_at(&draw_ops(&tree), sx, sy, sr),
        "the handle overlay is removed once the selection clears",
    );
}

/// `selectable_paragraph` と同様だが、テキストの上に余白を取り、フローティング
/// ツールバーが選択の*上*に収まるようにする。これで下のドラッグハンドルが隠れず、
/// ハンドルドラッグのテストで押下がツールバーボタンでなくハンドルに当たる。
/// テキストは y=88 付近。
fn selectable_paragraph_with_headroom() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::PaddingTop(Dimension::px(80.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, text);
    tree.element_set_text(text, "Hello world");
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, view, text)
}

#[test]
fn dragging_the_end_handle_extends_the_range() {
    let (mut tree, _view, text) = selectable_paragraph_with_headroom();
    // テキスト行付近（~y=88）で短い先頭範囲を Touch 選択する。
    tree.on_pointer_down_with_kind(2.0, 88.0, 0, PointerKind::Touch);
    tree.on_pointer_move(40.0, 88.0);
    tree.on_pointer_up(40.0, 88.0);
    let before = tree.selection().unwrap().range_within(text).unwrap();

    // end ハンドルを掴んでテキストの右端まで引っ張る。
    let handles = tree.selection_handles().expect("handles after selecting");
    tree.on_pointer_down(handles.end.knob_x, handles.end.knob_y);
    tree.on_pointer_move(398.0, 88.0);
    tree.on_pointer_up(398.0, 88.0);

    let after = tree.selection().unwrap().range_within(text).unwrap();
    assert_eq!(after.0, before.0, "the left edge stays put");
    assert!(after.1 > before.1, "dragging the end handle extends the range");
}

#[test]
fn dragging_the_start_handle_moves_the_left_edge() {
    let (mut tree, _view, text) = selectable_paragraph_with_headroom();
    // 最初の単語全体の範囲を Touch 選択する。
    tree.on_pointer_down_with_kind(2.0, 88.0, 0, PointerKind::Touch);
    tree.on_pointer_move(90.0, 88.0);
    tree.on_pointer_up(90.0, 88.0);
    let before = tree.selection().unwrap().range_within(text).unwrap();

    // start ハンドルを掴んで右へ引き、左側から範囲を縮める。
    let handles = tree.selection_handles().expect("handles after selecting");
    tree.on_pointer_down(handles.start.knob_x, handles.start.knob_y);
    tree.on_pointer_move(40.0, 88.0);
    tree.on_pointer_up(40.0, 88.0);

    let after = tree.selection().unwrap().range_within(text).unwrap();
    assert_eq!(after.1, before.1, "the right edge stays put");
    assert!(after.0 > before.0, "dragging the start handle moves the left edge in");
}

#[test]
fn long_press_starts_a_word_selection_with_handles_and_toolbar() {
    let (mut tree, _view, text) = selectable_paragraph();

    // 最初の単語 "Hello"（バイト 0..5）の内側を長押しする。
    tree.on_long_press(10.0, 8.0);

    let sel = tree.selection().expect("long-press selects a word");
    assert_eq!(
        sel.range_within(text),
        Some((0, 5)),
        "the word under the long-press is selected",
    );
    assert_eq!(tree.selected_text().as_deref(), Some("Hello"));
    assert!(
        tree.selection_handles().is_some(),
        "word selection raises drag handles",
    );
    assert!(
        tree.selection_toolbar().is_some(),
        "word selection raises the floating toolbar",
    );
}

#[test]
fn long_press_outside_a_selectable_region_selects_nothing() {
    let (mut tree, _view, _text) = selectable_paragraph();
    // 1 行のテキストよりはるか下、かつ（selectable な）view 端より外、
    // ビューポート右端も越えた地点 — 単語を固定するグリフが存在しない。
    tree.on_long_press(2000.0, 8.0);
    assert!(tree.selection().is_none(), "no word, no selection");
}

#[test]
fn a_collapsed_caret_raises_no_handles() {
    // 単一タップはキャレット（折りたたまれた選択）を落とすが、両端ハンドルは出さない。
    let (mut tree, _view, _text) = selectable_paragraph();
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_up(2.0, 8.0);
    assert!(
        tree.selection_handles().is_none(),
        "a collapsed caret shows no range handles",
    );
}
