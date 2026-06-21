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
    assert!(text.ends_with('…'), "expected trailing ellipsis, got {text:?}");
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
    assert!(text.ends_with('…'), "expected trailing ellipsis, got {text:?}");
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
    assert!(!text.ends_with('…'), "no max-lines ⇒ no ellipsis, got {text:?}");
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

    let (ex, ey, ew, eh) = tree
        .element_layout_rect(ifc)
        .expect("IFC layout");
    let hit_x = ex + ew * 0.85;
    let hit_y = ey + eh * 0.5;
    let hit = tree.hit_test(hit_x, hit_y);
    assert_eq!(
        hit,
        Some(inline),
        "point in inline span should resolve to inline text element"
    );
}
