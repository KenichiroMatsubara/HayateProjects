use hayate_core::{Dimension, ElementKind, ElementTree, StyleProp, TextOverflowValue};

/// 狭い（120px）IFC ボックス内で複数行に折り返す長文。
const LONG_TEXT: &str =
    "The quick brown fox jumps over the lazy dog near the river bank early today";

/// 指定幅の単一テキスト要素 IFC を構築し、(tree, ifc id) を返す。
fn narrow_ifc(width: f32, extra: &[StyleProp]) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let ifc = tree.element_create(2, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(width, 600.0);
    let mut props = vec![StyleProp::Width(Dimension::px(width))];
    props.extend_from_slice(extra);
    tree.element_set_style(ifc, &props);
    tree.element_append_child(root, ifc);
    tree.element_set_text(ifc, LONG_TEXT);
    tree.render(0.0);
    (tree, ifc)
}

#[test]
fn long_text_wraps_to_more_than_three_lines_without_max_lines() {
    // ベースライン: フィクスチャが実際に max-lines の上限を超えて折り返さないと、
    // 以降の切り詰めテストが空虚に成立してしまう。
    let (tree, ifc) = narrow_ifc(120.0, &[]);
    let lines = tree.test_text_line_count(ifc).expect("shaped IFC");
    assert!(lines > 3, "expected >3 wrapped lines, got {lines}");
}

#[test]
fn max_lines_two_silently_clips_to_two_lines() {
    let (tree, ifc) = narrow_ifc(120.0, &[StyleProp::MaxLines(2)]);
    assert_eq!(tree.test_text_line_count(ifc), Some(2));
    let text = tree.test_shaped_text(ifc).expect("shaped IFC");
    assert!(
        !text.ends_with('…'),
        "clip is the default — no ellipsis, got {text:?}"
    );
}

#[test]
fn max_lines_one_with_ellipsis_fits_one_line_and_appends_ellipsis() {
    let (tree, ifc) = narrow_ifc(
        120.0,
        &[
            StyleProp::MaxLines(1),
            StyleProp::TextOverflow(TextOverflowValue::Ellipsis),
        ],
    );
    assert_eq!(tree.test_text_line_count(ifc), Some(1));
    let text = tree.test_shaped_text(ifc).expect("shaped IFC");
    assert!(
        text.ends_with('…'),
        "expected trailing ellipsis, got {text:?}"
    );
}

#[test]
fn max_lines_three_with_ellipsis_keeps_three_lines_and_appends_ellipsis() {
    let (tree, ifc) = narrow_ifc(
        120.0,
        &[
            StyleProp::MaxLines(3),
            StyleProp::TextOverflow(TextOverflowValue::Ellipsis),
        ],
    );
    assert_eq!(tree.test_text_line_count(ifc), Some(3));
    let text = tree.test_shaped_text(ifc).expect("shaped IFC");
    assert!(
        text.ends_with('…'),
        "expected trailing ellipsis, got {text:?}"
    );
}

#[test]
fn ellipsis_without_max_lines_has_no_effect() {
    let baseline = {
        let (tree, ifc) = narrow_ifc(120.0, &[]);
        tree.test_text_line_count(ifc).expect("shaped IFC")
    };
    let (tree, ifc) = narrow_ifc(
        120.0,
        &[StyleProp::TextOverflow(TextOverflowValue::Ellipsis)],
    );
    assert_eq!(
        tree.test_text_line_count(ifc),
        Some(baseline),
        "text-overflow without max-lines must not truncate"
    );
    let text = tree.test_shaped_text(ifc).expect("shaped IFC");
    assert!(
        !text.ends_with('…'),
        "no max-lines ⇒ no ellipsis, got {text:?}"
    );
}

/// 回帰: `max_lines` を**テキストを内包するボックス**（todo カードのタイトルは
/// `<button>` に `maxLines:1` を置き、テキストはその子）へ宣言したとき、Canvas でも
/// 子テキストがクランプされる。DOM Mode はカタログ `domExtras` でボタンへ
/// `-webkit-line-clamp` を載せてクランプするため、宣言場所が IFC ルート自身でなくても
/// 両レンダラーが一致しなければならない（Canvas だけ折り返してカードが伸びる乖離）。
#[test]
fn max_lines_on_containing_box_clamps_child_text() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    // タイトルボタン相当: クランプ系プロップはここ（ブロックボックス）に載る。
    let box_id = tree.element_create(2, ElementKind::Button);
    let text = tree.element_create(3, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(120.0, 600.0);
    tree.element_set_style(
        box_id,
        &[
            StyleProp::Width(Dimension::px(120.0)),
            StyleProp::MaxLines(1),
            StyleProp::TextOverflow(TextOverflowValue::Ellipsis),
        ],
    );
    tree.element_append_child(root, box_id);
    tree.element_append_child(box_id, text);
    tree.element_set_text(text, LONG_TEXT);
    tree.render(0.0);

    assert_eq!(
        tree.test_text_line_count(text),
        Some(1),
        "max-lines declared on the containing box must clamp the child text"
    );
    let shaped = tree.test_shaped_text(text).expect("shaped IFC");
    assert!(
        shaped.ends_with('…'),
        "expected trailing ellipsis, got {shaped:?}"
    );
}

/// 一般化①: クランプはテキストを**直接内包するブロック**（包含ブロック）に置けば、
/// 階層が深くても効く。`view > view(maxLines) > text`。
#[test]
fn max_lines_on_inner_containing_block_clamps() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let outer = tree.element_create(2, ElementKind::View);
    let inner = tree.element_create(3, ElementKind::View);
    let text = tree.element_create(4, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(120.0, 600.0);
    tree.element_set_style(outer, &[StyleProp::Width(Dimension::px(120.0))]);
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(120.0)),
            StyleProp::MaxLines(1),
            StyleProp::TextOverflow(TextOverflowValue::Ellipsis),
        ],
    );
    tree.element_append_child(root, outer);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, text);
    tree.element_set_text(text, LONG_TEXT);
    tree.render(0.0);
    assert_eq!(tree.test_text_line_count(text), Some(1));
}

/// 一般化②（DOM パリティ）: クランプは間に挟まったブロックを**貫通しない**。
/// `view(maxLines) > view > text` では子テキストはクランプされない。
/// CSS `-webkit-line-clamp` も中間ブロックの行は畳まないので両者一致。
#[test]
fn max_lines_does_not_pierce_intermediate_block() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let outer = tree.element_create(2, ElementKind::View);
    let inner = tree.element_create(3, ElementKind::View);
    let text = tree.element_create(4, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(120.0, 600.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(120.0)),
            StyleProp::MaxLines(1),
        ],
    );
    tree.element_set_style(inner, &[StyleProp::Width(Dimension::px(120.0))]);
    tree.element_append_child(root, outer);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, text);
    tree.element_set_text(text, LONG_TEXT);
    tree.render(0.0);
    let lines = tree.test_text_line_count(text).expect("shaped IFC");
    assert!(
        lines > 1,
        "must not clamp through an intermediate block, got {lines}"
    );
}

/// 一般化③: IFC ルートテキスト自身の `max_lines` は内包ボックスの宣言に勝つ。
#[test]
fn text_own_max_lines_overrides_containing_box() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let box_id = tree.element_create(2, ElementKind::Button);
    let text = tree.element_create(3, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(120.0, 600.0);
    tree.element_set_style(
        box_id,
        &[
            StyleProp::Width(Dimension::px(120.0)),
            StyleProp::MaxLines(1),
        ],
    );
    tree.element_set_style(text, &[StyleProp::MaxLines(3)]);
    tree.element_append_child(root, box_id);
    tree.element_append_child(box_id, text);
    tree.element_set_text(text, LONG_TEXT);
    tree.render(0.0);
    assert_eq!(
        tree.test_text_line_count(text),
        Some(3),
        "text's own max-lines wins"
    );
}

#[test]
fn ifc_root_shapes_concatenated_inline_text() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let ifc = tree.element_create(2, ElementKind::Text);
    let inline = tree.element_create(3, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 100.0);
    tree.element_append_child(root, ifc);
    tree.element_append_child(ifc, inline);
    tree.element_set_text(ifc, "Hi ");
    tree.element_set_text(inline, "there");
    tree.render(0.0);

    let text = tree.element_get_text(ifc);
    assert_eq!(text, "Hi ");
    tree.render(0.0);
    assert!(
        tree.element_layout_rect(ifc).is_some(),
        "IFC root should have box geometry"
    );
}

#[test]
fn hit_test_resolves_inline_text_element_inside_ifc() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let ifc = tree.element_create(11, ElementKind::Text);
    let inline = tree.element_create(12, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(hayate_core::Dimension::px(400.0)),
            StyleProp::Height(hayate_core::Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(root, ifc);
    tree.element_append_child(ifc, inline);
    tree.element_set_text(ifc, "AAAA");
    tree.element_set_text(inline, "BBBB");
    tree.render(0.0);

    let (ex, ey, ew, eh) = tree.element_layout_rect(ifc).expect("IFC layout");
    let hit_x = ex + ew * 0.85;
    let hit_y = ey + eh * 0.5;
    let hit = tree.hit_test(hit_x, hit_y);
    assert_eq!(
        hit,
        Some(inline),
        "point in inline span should resolve to inline text element"
    );
}
