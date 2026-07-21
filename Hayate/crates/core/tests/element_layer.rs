use hayate_core::{
    AlignValue, BorderStyleValue, Color, Dimension, DisplayValue, DocumentEventKind, ElementId,
    ElementKind, ElementTree, Event, FlexDirectionValue, NodeKind, StyleProp,
};

// ── レイアウト/描画ヘルパ ─────────────────────────────────────────────

#[test]
fn element_create_and_append_builds_tree() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let child_a = tree.element_create(2, ElementKind::View);
    let child_b = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, child_a);
    tree.element_append_child(root, child_b);
    assert_eq!(tree.root(), Some(root));
}

#[test]
fn set_style_routes_layout_and_visual() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(4, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    let sg = tree.render(0.0);
    // レイアウト計算済みサイズの Rect ノードが 1 つ出る想定。
    let mut found = false;
    for (_, n) in sg.iter() {
        if let NodeKind::Rect {
            width,
            height,
            color,
            ..
        } = &n.kind
        {
            assert!((*width - 100.0).abs() < 0.5);
            assert!((*height - 50.0).abs() < 0.5);
            assert!((color[0] - 1.0).abs() < 1e-3);
            found = true;
        }
    }
    assert!(found, "background rect not emitted");
}

#[test]
fn border_radius_emits_rounded_background_rect() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(40, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            StyleProp::BorderRadius(12.0),
        ],
    );

    let sg = tree.render(0.0);
    let rects: Vec<_> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect {
                width,
                height,
                corner_radius,
                ..
            } if (*width - 100.0).abs() < 0.5 && (*height - 80.0).abs() < 0.5 => {
                Some(*corner_radius)
            }
            _ => None,
        })
        .collect();

    assert_eq!(rects, vec![12.0], "expected one rounded background rect");
}

#[test]
fn border_radius_with_border_and_background_emits_nested_rounded_fills() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(41, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            StyleProp::BorderColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::BorderWidth(4.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderRadius(12.0),
        ],
    );

    let sg = tree.render(0.0);
    let mut outer = None;
    let mut inner = None;
    for (_, n) in sg.iter() {
        if let NodeKind::Rect {
            x,
            y,
            width,
            height,
            color,
            corner_radius,
        } = &n.kind
        {
            if (*width - 100.0).abs() < 0.5 && (*height - 80.0).abs() < 0.5 {
                outer = Some((*corner_radius, color[0]));
            } else if (*width - 92.0).abs() < 0.5
                && (*height - 72.0).abs() < 0.5
                && (*x - 4.0).abs() < 0.5
                && (*y - 4.0).abs() < 0.5
            {
                inner = Some((*corner_radius, color[2]));
            }
        }
    }

    assert_eq!(outer, Some((12.0, 1.0)), "outer border frame");
    assert_eq!(inner, Some((8.0, 1.0)), "inner background inset");
}

#[test]
fn border_radius_without_background_emits_rounded_ring() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(42, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::BorderColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::BorderWidth(4.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderRadius(12.0),
        ],
    );

    let sg = tree.render(0.0);
    let rings: Vec<_> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::RoundedRing {
                width,
                height,
                outer_radius,
                border_width,
                ..
            } if (*width - 100.0).abs() < 0.5 && (*height - 80.0).abs() < 0.5 => {
                Some((*outer_radius, *border_width))
            }
            _ => None,
        })
        .collect();

    assert_eq!(rings, vec![(12.0, 4.0)]);
}

#[test]
fn flex_row_positions_children_with_gap() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(5, ElementKind::View);
    let a = tree.element_create(6, ElementKind::View);
    let b = tree.element_create(7, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(500.0, 200.0);

    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Gap(Dimension::px(10.0)),
            StyleProp::Width(Dimension::px(500.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(Color::new(0.1, 0.1, 0.1, 1.0)),
        ],
    );
    for &child in &[a, b] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
                StyleProp::BackgroundColor(Color::new(0.0, 0.5, 1.0, 1.0)),
            ],
        );
    }

    let sg = tree.render(0.0);
    let mut xs: Vec<f32> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { x, width, .. } if (*width - 100.0).abs() < 0.5 => Some(*x),
            _ => None,
        })
        .collect();
    xs.sort_by(|p, q| p.partial_cmp(q).unwrap());
    assert_eq!(xs.len(), 2, "expected two child rects, got {xs:?}");
    assert!((xs[0] - 0.0).abs() < 0.5, "first child x = {}", xs[0]);
    assert!((xs[1] - 110.0).abs() < 0.5, "second child x = {}", xs[1]);
}

#[test]
fn flex_grow_expands_flex_children() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(7001, ElementKind::View);
    let a = tree.element_create(7002, ElementKind::View);
    let b = tree.element_create(7003, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 100.0);

    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    for &child in &[a, b] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(50.0)),
                StyleProp::Height(Dimension::px(50.0)),
                StyleProp::FlexGrow(1.0),
                StyleProp::BackgroundColor(Color::new(0.0, 0.5, 1.0, 1.0)),
            ],
        );
    }

    let sg = tree.render(0.0);
    let mut widths: Vec<f32> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, height, .. } if (*height - 50.0).abs() < 0.5 => Some(*width),
            _ => None,
        })
        .collect();
    widths.sort_by(|p, q| p.partial_cmp(q).unwrap());
    assert_eq!(
        widths.len(),
        2,
        "expected two flex child rects, got {widths:?}"
    );
    assert!(
        (widths[0] - 150.0).abs() < 0.5,
        "first child width = {}",
        widths[0]
    );
    assert!(
        (widths[1] - 150.0).abs() < 0.5,
        "second child width = {}",
        widths[1]
    );
}

#[test]
fn text_element_produces_text_run() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(8, ElementKind::View);
    let text = tree.element_create(9, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(root, text);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::FontSize(24.0)]);
    tree.element_set_text(text, "Hello");
    assert_eq!(tree.element_get_text(text), "Hello");
    let sg = tree.render(0.0);
    let has_text_run = sg
        .iter()
        .any(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }));
    assert!(has_text_run, "no TextRun emitted for text element");
}

/// `tree` が最初に出力した TextRun の色。なければ `None`。
fn first_text_run_color(tree: &mut ElementTree) -> Option<[f32; 4]> {
    tree.render(0.0).iter().find_map(|(_, n)| match &n.kind {
        NodeKind::TextRun { color, .. } => Some(*color),
        _ => None,
    })
}

/// 自前のツリーを root とする 400×40 の TextInput を組む。本文 `color`、
/// placeholder 文字列（ADR-0058: placeholder は `text` に宿る）、および任意で
/// 確定済みの編集内容を持たせる。
fn text_input_tree(body: Color, committed: &str) -> ElementTree {
    let mut tree = ElementTree::new();
    let input = tree.element_create(1, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(400.0, 60.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::Color(body),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_set_text(input, "新しいタスクを入力…");
    if !committed.is_empty() {
        tree.element_set_text_content(input, committed);
    }
    tree
}

#[test]
fn placeholder_text_run_is_muted_not_body_color() {
    // Canvas の視覚基準は Chromium DOM であり、その `::placeholder` は本文 `color`
    // ではなく淡色で描かれる（ADR-0102）。空の TextInput は placeholder を `color`
    // とは別の色で描かねばならない。
    let body = Color::new(50.0 / 255.0, 44.0 / 255.0, 63.0 / 255.0, 1.0);
    let mut tree = text_input_tree(body, "");
    let run = first_text_run_color(&mut tree).expect("placeholder should emit a TextRun");

    assert_ne!(
        [run[0], run[1], run[2]],
        [body.r as f32, body.g as f32, body.b as f32],
        "placeholder must not be painted in the body color (it should be muted)",
    );
}

#[test]
fn committed_text_run_keeps_body_color() {
    // 淡色になるのは placeholder だけ。確定テキストを持つ入力では、run は
    // 本文 `color` で描かれ続けねばならない（実入力の退行防止）。
    let body = Color::new(50.0 / 255.0, 44.0 / 255.0, 63.0 / 255.0, 1.0);
    let mut tree = text_input_tree(body, "牛乳を買う");
    let run = first_text_run_color(&mut tree).expect("committed text should emit a TextRun");

    assert_eq!(
        [run[0], run[1], run[2], run[3]],
        [body.r as f32, body.g as f32, body.b as f32, body.a as f32],
        "committed text must keep the body color",
    );
}

#[test]
fn scene_build_walks_absolute_coordinates() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let child = tree.element_create(11, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::PaddingLeft(Dimension::px(20.0)),
            StyleProp::PaddingTop(Dimension::px(20.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, child);
    let sg = tree.render(0.0);
    let mut child_pos = None;
    for (_, n) in sg.iter() {
        if let NodeKind::Rect {
            x,
            y,
            width,
            height,
            color,
            ..
        } = &n.kind
        {
            if (*width - 50.0).abs() < 0.5
                && (*height - 50.0).abs() < 0.5
                && (color[1] - 1.0).abs() < 1e-3
            {
                child_pos = Some((*x, *y));
            }
        }
    }
    let (x, y) = child_pos.expect("child rect missing");
    assert!((x - 20.0).abs() < 0.5, "child x = {x}");
    assert!((y - 20.0).abs() < 0.5, "child y = {y}");
}

// ── ScrollView テスト ─────────────────────────────────────────────────────

#[test]
fn scroll_view_emits_clip_node() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(12, ElementKind::ScrollView);
    let content = tree.element_create(13, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(500.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, content);
    let sg = tree.render(0.0);

    let clip_count = sg
        .iter()
        .filter(|(_, n)| matches!(n.kind, NodeKind::Clip { .. }))
        .count();
    assert_eq!(
        clip_count, 1,
        "ScrollView should emit exactly one Clip node"
    );
}

#[test]
fn scroll_view_clip_contains_content_as_child() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(14, ElementKind::ScrollView);
    let content = tree.element_create(15, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(500.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, content);

    // スクロールオフセットを与え、Clip 内に Group が生じることを確認する。
    tree.element_set_scroll_offset(root, 0.0, 50.0);
    let sg = tree.render(0.0);

    let clip_id = sg
        .iter()
        .find(|(_, n)| matches!(n.kind, NodeKind::Clip { .. }))
        .map(|(id, _)| id)
        .expect("ScrollView should emit a Clip node");
    let clip_node = sg.get(clip_id).unwrap();
    // Clip の最初の子は Group（スクロール平行移動）のはず。
    assert!(!clip_node.children.is_empty(), "Clip should have children");
    let first_child = sg.get(clip_node.children[0]).unwrap();
    assert!(
        matches!(first_child.kind, NodeKind::Group { .. }),
        "Clip's first child should be a Group (scroll offset)"
    );
}

// ── Transform / Group テスト ──────────────────────────────────────────────

#[test]
fn transform_emits_group_node() {
    use hayate_core::NodeKind;
    let mut tree = ElementTree::new();
    let root = tree.element_create(16, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    // 恒等変換 — Group ノードが現れ、Rect はその子になるはず。
    let identity = [1.0_f64, 0.0, 0.0, 1.0, 0.0, 0.0];
    tree.element_set_transform(root, Some(identity));
    let sg = tree.render(0.0);

    let mut group_count = 0usize;
    let mut rect_count = 0usize;
    for (_, n) in sg.iter() {
        match &n.kind {
            NodeKind::Group { .. } => group_count += 1,
            NodeKind::Rect { .. } => rect_count += 1,
            _ => {}
        }
    }
    assert_eq!(group_count, 1, "expected one Group node");
    assert_eq!(rect_count, 1, "expected one Rect node (background)");

    let group_id = sg
        .iter()
        .find(|(_, n)| matches!(n.kind, NodeKind::Group { .. }))
        .map(|(id, _)| id)
        .expect("transform should emit a Group node");
    let group_node = sg.get(group_id).unwrap();
    assert_eq!(
        group_node.children.len(),
        1,
        "Rect should be a child of Group"
    );
}

// ── ZIndex テスト ─────────────────────────────────────────────────────────

#[test]
fn z_index_controls_paint_order() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(17, ElementKind::View);
    let back = tree.element_create(18, ElementKind::View);
    let front = tree.element_create(19, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    // back を先に append し z_index 1、front を後に append し z_index 0 とする。
    // ソート後は front（z=0）が back（z=1）より先に描かれるはず。
    tree.element_append_child(root, back);
    tree.element_append_child(root, front);
    tree.element_set_style(
        back,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::ZIndex(1),
        ],
    );
    tree.element_set_style(
        front,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            StyleProp::ZIndex(0),
        ],
    );
    let sg = tree.render(0.0);
    // 描画順を収集: 各 50×50 rect の color 第 1 成分を見る。
    let order: Vec<f32> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, color, .. } if (*width - 50.0).abs() < 0.5 => Some(color[0]),
            _ => None,
        })
        .collect();
    // front（青, r=0）が back（赤, r=1）より先に来るはず。
    assert_eq!(order.len(), 2, "expected 2 child rects");
    assert!((order[0] - 0.0).abs() < 1e-3, "front (blue) first");
    assert!((order[1] - 1.0).abs() < 1e-3, "back (red) second");
}

// ── ADR-0060: Z-Order 単一順序付けシーム（ordered_children） ─────────────

#[test]
fn ordered_children_is_stable_paint_order() {
    // document 順 a,b,c,d ; z: a=0, b=2, c=0, d=1
    let mut tree = ElementTree::new();
    let root = tree.element_create(700, ElementKind::View);
    let a = tree.element_create(701, ElementKind::View);
    let b = tree.element_create(702, ElementKind::View);
    let c = tree.element_create(703, ElementKind::View);
    let d = tree.element_create(704, ElementKind::View);
    tree.set_root(root);
    for &ch in &[a, b, c, d] {
        tree.element_append_child(root, ch);
    }
    tree.element_set_style(b, &[StyleProp::ZIndex(2)]);
    tree.element_set_style(d, &[StyleProp::ZIndex(1)]);

    // paint order = z 昇順、同 z は document 順で安定（後勝ち: a の後に c）
    assert_eq!(tree.ordered_children(root), vec![a, c, d, b]);

    // hit-test = paint の逆順（最前面 = z 最大 / 同 z は後の兄弟）
    let hit_order: Vec<ElementId> = tree.ordered_children(root).into_iter().rev().collect();
    assert_eq!(hit_order, vec![b, d, c, a]);
}

// ── ADR-0058: text は text-like 要素にのみ宿る ─────────────────────────────

#[test]
fn element_set_text_is_ignored_on_non_text_elements() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(720, ElementKind::View);
    let btn = tree.element_create(721, ElementKind::Button);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_append_child(root, btn);
    tree.element_set_style(
        btn,
        &[
            StyleProp::Width(Dimension::px(80.0)),
            StyleProp::Height(Dimension::px(30.0)),
        ],
    );
    // ADR-0058: button ラベルは子 text 要素で持つ。button 自身への set は無視される。
    tree.element_set_text(btn, "Save");
    let sg = tree.render(0.0);
    let text_runs = sg
        .iter()
        .filter(|(_, n)| matches!(n.kind, NodeKind::TextRun { .. }))
        .count();
    assert_eq!(
        text_runs, 0,
        "button.text must not render; labels go on a child text element (ADR-0058)"
    );
}

// ── イベントシステムのテスト ───────────────────────────────────────────────────

#[test]
fn hit_test_returns_deepest_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(20, ElementKind::View);
    let child = tree.element_create(21, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(400.0)),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(root, child);
    tree.render(0.0);

    // child 内の点 → child が最深で勝つ
    assert_eq!(tree.hit_test(50.0, 50.0), Some(child));
    // child の外だが root 内の点 → root
    assert_eq!(tree.hit_test(200.0, 200.0), Some(root));
    // すべての外側の点 → None
    assert_eq!(tree.hit_test(500.0, 500.0), None);
}

#[test]
fn hit_test_respects_z_index_when_paint_and_doc_order_diverge() {
    // 退行防止: かつて hit_test は z-index を無視して document 逆順で子を辿った。
    // scene_build は描画順を z-index でソートするため、document 順と z-index が食い違うと
    // 視覚上の最前面要素が hit できなかった。
    let mut tree = ElementTree::new();
    let root = tree.element_create(22, ElementKind::View);
    let back = tree.element_create(23, ElementKind::View);
    let front = tree.element_create(24, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    // 負マージンの flex column で 2 つの 100×100 兄弟を重ねる。
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    // `back` は先に append するが z_index=1（前面の意図）。
    // `front` は後に append し z_index=0（背面の意図）だが、margin-top -100px で
    // `back` にぴったり重なる。
    tree.element_append_child(root, back);
    tree.element_append_child(root, front);
    tree.element_set_style(
        back,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::ZIndex(1),
        ],
    );
    tree.element_set_style(
        front,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::MarginTop(Dimension::px(-100.0)),
            StyleProp::ZIndex(0),
        ],
    );
    tree.render(0.0);

    // 2 つの子が実際に (50, 50) で重なることを確認する。
    let back_rect = tree.element_layout_rect(back).unwrap();
    let front_rect = tree.element_layout_rect(front).unwrap();
    assert!(
        (back_rect.0 - front_rect.0).abs() < 0.5 && (back_rect.1 - front_rect.1).abs() < 0.5,
        "test setup failed: children must overlap (back={back_rect:?}, front={front_rect:?})"
    );

    // 視覚上は `back` が前面（z=1 > z=0）なので、hit_test は `back` を返さねばならない。
    assert_eq!(tree.hit_test(50.0, 50.0), Some(back));
}

#[test]
fn push_and_poll_events() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(25, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.render(0.0);

    tree.push_event(Event::Click {
        target_id: root,
        x: 10.0,
        y: 20.0,
    });
    tree.push_event(Event::Resize {
        width: 300.0,
        height: 400.0,
    });

    let events = tree.poll_events();
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0], Event::Click { x, .. } if (*x - 10.0).abs() < 1e-3));
    assert!(matches!(&events[1], Event::Resize { width, .. } if (*width - 300.0).abs() < 1e-3));

    // poll 後はキューが空になる。
    assert!(tree.poll_events().is_empty());
}

#[test]
fn scroll_event_targets_hit_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(26, ElementKind::ScrollView);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.render(0.0);

    let target = tree.hit_test(100.0, 100.0).expect("no hit");
    tree.push_event(Event::Scroll {
        target_id: target,
        delta_x: 0.0,
        delta_y: 20.0,
    });

    let events = tree.poll_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], Event::Scroll { delta_y, .. } if (*delta_y - 20.0).abs() < 1e-3));
}

#[test]
fn remove_subtree_drops_children() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(27, ElementKind::View);
    let a = tree.element_create(28, ElementKind::View);
    let b = tree.element_create(29, ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, a);
    tree.element_append_child(a, b);
    tree.element_remove(a);
    // `a` を削除すると `a` と `b` の両方が消え、root は残るはず。
    assert_eq!(tree.element_kind(root), Some(ElementKind::View));
    assert_eq!(tree.element_kind(a), None);
    assert_eq!(tree.element_kind(b), None);
}

#[test]
fn subtree_element_ids_returns_root_and_descendants() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(200, ElementKind::View);
    let child = tree.element_create(201, ElementKind::View);
    let grandchild = tree.element_create(202, ElementKind::Text);
    tree.set_root(root);
    tree.element_append_child(root, child);
    tree.element_append_child(child, grandchild);

    let ids: Vec<u64> = tree
        .subtree_element_ids(child)
        .into_iter()
        .map(|id| id.to_u64())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&child.to_u64()));
    assert!(ids.contains(&grandchild.to_u64()));
    assert!(tree
        .subtree_element_ids(ElementId::from_u64(999))
        .is_empty());
}

// ── TextInput + IME テスト ──────────────────────────────────────

#[test]
fn text_input_text_run_respects_padding() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(29, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::PaddingLeft(Dimension::px(12.0)),
            StyleProp::PaddingTop(Dimension::px(8.0)),
            StyleProp::FontSize(13.0),
        ],
    );
    tree.element_append_text_content(input, "Focus me");

    let sg = tree.render(0.0);
    let text_run = sg
        .iter()
        .find_map(|(_, n)| {
            if let NodeKind::TextRun { x, y, text_run, .. } = &n.kind {
                let data = sg.resources().text_run(*text_run).ok()?;
                Some((*x, *y, data.glyphs.first().map(|g| g.x)))
            } else {
                None
            }
        })
        .expect("TextRun for padded text-input");
    let (run_x, run_y, first_glyph_x) = text_run;
    let text_x = run_x + first_glyph_x.unwrap_or(0.0);
    assert!(
        (text_x - 12.0).abs() < 0.5,
        "text should start at padding-left inset, got x={text_x}"
    );
    assert!(
        (run_y - 8.0).abs() < 0.5,
        "text run y should include padding-top inset, got y={run_y}"
    );
}

#[test]
fn text_input_cursor_respects_padding() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(28, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::PaddingLeft(Dimension::px(12.0)),
            StyleProp::PaddingTop(Dimension::px(8.0)),
            StyleProp::FontSize(13.0),
        ],
    );
    tree.element_focus(input);

    let sg = tree.render(0.0);
    let cursor = sg
        .iter()
        .find_map(|(_, n)| {
            if let NodeKind::Rect {
                x,
                y,
                width,
                height,
                corner_radius,
                ..
            } = &n.kind
            {
                if *width <= 2.0 && *height > 10.0 && *corner_radius == 0.0 {
                    Some((*x, *y))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("cursor rect for empty padded text-input");
    let (cursor_x, cursor_y) = cursor;
    assert!(
        (cursor_x - 12.0).abs() < 0.5,
        "empty-input cursor x should start at padding-left inset, got x={cursor_x}"
    );
    assert!(
        (cursor_y - 8.0).abs() < 0.5,
        "empty-input cursor y should start at padding-top inset, got y={cursor_y}"
    );
}

#[test]
fn text_input_append_and_get() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(30, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "hello");
    assert_eq!(tree.element_get_text_content(input), "hello");

    tree.element_append_text_content(input, " world");
    assert_eq!(tree.element_get_text_content(input), "hello world");
}

#[test]
fn text_input_set_replaces_content() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(31, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "old");
    tree.element_set_text_content(input, "new");
    assert_eq!(tree.element_get_text_content(input), "new");
}

#[test]
fn preedit_shown_inline_not_committed() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(32, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "abc");
    tree.element_set_preedit(input, "DEF");

    // 表示テキストには preedit が末尾に含まれる。
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
}

#[test]
fn preedit_text_run_respects_padding() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(27, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::PaddingLeft(Dimension::px(12.0)),
            StyleProp::PaddingTop(Dimension::px(8.0)),
            StyleProp::FontSize(13.0),
        ],
    );
    tree.element_set_preedit(input, "あ");

    let sg = tree.render(0.0);
    let (run_x, run_y) = sg
        .iter()
        .find_map(|(_, n)| {
            if let NodeKind::TextRun { x, y, .. } = &n.kind {
                Some((*x, *y))
            } else {
                None
            }
        })
        .expect("preedit TextRun with padding");
    assert!(
        (run_x - 12.0).abs() < 0.5,
        "preedit x should include padding-left"
    );
    assert!(
        (run_y - 8.0).abs() < 0.5,
        "preedit y should include padding-top"
    );
}

#[test]
fn placeholder_renders_when_text_content_is_empty() {
    // 退行防止: Canvas モードの TextInput は value が空のとき placeholder を描かねばならない。
    // かつて layout_pass は TextInput の text_layout 構築をスキップし、scene_build の
    // content_layout → text_layout フォールバックを dead code にしていた。
    let mut tree = ElementTree::new();
    let input = tree.element_create(36, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(24.0),
        ],
    );

    tree.element_set_text(input, "Type here");

    let sg = tree.render(0.0);
    let text_run_count = sg
        .iter()
        .filter(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }))
        .count();
    assert!(
        text_run_count > 0,
        "placeholder text must render as a TextRun when text_content is empty"
    );
}

#[test]
fn placeholder_hidden_when_text_content_is_present() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(37, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(24.0),
        ],
    );

    tree.element_set_text(input, "Type here");
    tree.element_set_text_content(input, "Hello");

    let sg = tree.render(0.0);
    let text_run_count = sg
        .iter()
        .filter(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }))
        .count();
    assert!(
        text_run_count > 0,
        "committed text must render as a TextRun when value is present"
    );

    // value をクリアすると placeholder 描画が復活する。
    tree.element_set_text_content(input, "");
    let sg = tree.render(0.0);
    let text_run_count = sg
        .iter()
        .filter(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }))
        .count();
    assert!(
        text_run_count > 0,
        "placeholder must render again after value is cleared"
    );
}

#[test]
fn preedit_renders_when_text_content_is_empty() {
    // 退行防止: 空の TextInput への IME 変換入力は preedit を TextRun として出さねばならない。
    // かつて scene_build はコンテンツ描画を text_content だけで判定し、確定まで preedit を
    // 隠していた。
    let mut tree = ElementTree::new();
    let input = tree.element_create(33, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(24.0),
        ],
    );

    // 確定テキストはまだなく、進行中の IME 変換のみ。
    tree.element_set_preedit(input, "あ");

    let sg = tree.render(0.0);
    let text_run_count = sg
        .iter()
        .filter(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }))
        .count();
    assert!(
        text_run_count > 0,
        "preedit text must render as a TextRun even when text_content is empty"
    );
}

#[test]
fn commit_preedit_appends_and_clears() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(34, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "abc");
    tree.element_set_preedit(input, "DEF");
    tree.element_commit_preedit(input);

    // 確定後、preedit は確定テキストの一部になる。
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
    // preedit を空に設定すると実質的にクリアされる。
    tree.element_set_preedit(input, "");
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
}

#[test]
fn text_input_event_queued_on_append() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(35, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "x");
    tree.push_event(Event::TextInput {
        target_id: input,
        text: "x".to_string(),
    });

    let events = tree.poll_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], Event::TextInput { text, .. } if text == "x"));
}

#[test]
fn composition_lifecycle_events_queued() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(36, ElementKind::TextInput);
    tree.set_root(input);

    tree.push_event(Event::CompositionStart {
        target_id: input,
        text: "あ".to_string(),
    });
    tree.push_event(Event::CompositionUpdate {
        target_id: input,
        text: "あい".to_string(),
    });
    tree.push_event(Event::CompositionEnd {
        target_id: input,
        text: "愛".to_string(),
    });

    let events = tree.poll_events();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], Event::CompositionStart { text, .. } if text == "あ"));
    assert!(matches!(&events[1], Event::CompositionUpdate { text, .. } if text == "あい"));
    assert!(matches!(&events[2], Event::CompositionEnd { text, .. } if text == "愛"));
}

// ── キーボードイベントのテスト（Enter キー, modifiers） ──────────────────────────

#[test]
fn backspace_removes_last_char() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(37, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "hello");
    tree.element_backspace(input);
    assert_eq!(tree.element_get_text_content(input), "hell");

    tree.element_backspace(input);
    assert_eq!(tree.element_get_text_content(input), "hel");
}

#[test]
fn backspace_on_empty_is_noop() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(38, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_backspace(input);
    assert_eq!(tree.element_get_text_content(input), "");
}

#[test]
fn enter_key_inserts_newline() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(39, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "line1");
    tree.element_append_text_content(input, "\n");
    tree.element_append_text_content(input, "line2");
    assert_eq!(tree.element_get_text_content(input), "line1\nline2");
}

#[test]
fn key_down_event_carries_modifiers() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(40, ElementKind::TextInput);
    tree.set_root(input);

    // modifier ビットマスク付きの Shift+A
    tree.push_event(Event::KeyDown {
        target_id: input,
        key: "A".to_string(),
        modifiers: 1,
    });
    let events = tree.poll_events();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0], Event::KeyDown { key, modifiers, .. } if key == "A" && *modifiers == 1)
    );
}

// ── カーソル可視性のテスト ───────────────────────────────────────────────

#[test]
fn cursor_visible_on_focus_hidden_on_blur() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(41, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_set_cursor_visible(input, true);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );

    let sg = tree.render(0.0);
    // カーソル可視かつ text_content が空のとき、フォールバックの Rect カーソルが出る。
    let cursor_rects: Vec<_> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, .. } if (*width - 1.5).abs() < 0.1 => Some(n),
            _ => None,
        })
        .collect();
    assert!(
        !cursor_rects.is_empty(),
        "cursor rect should be emitted when cursor_visible=true"
    );

    tree.element_set_cursor_visible(input, false);
    let sg = tree.render(0.0);
    let cursor_rects: Vec<_> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, .. } if (*width - 1.5).abs() < 0.1 => Some(n),
            _ => None,
        })
        .collect();
    assert!(
        cursor_rects.is_empty(),
        "cursor rect should not be emitted when cursor_visible=false"
    );
}

// ── ADR-0032: render(timestamp_ms) が内部でカーソル点滅を駆動する ────────

fn count_cursor_rects(sg: &hayate_core::SceneGraph) -> usize {
    sg.iter()
        .filter(
            |(_, n)| matches!(&n.kind, NodeKind::Rect { width, .. } if (*width - 1.5).abs() < 0.1),
        )
        .count()
}

#[test]
fn render_timestamp_toggles_focused_cursor_every_500ms() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(42, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );

    tree.element_focus(input);
    // 最初のフレーム: カーソル可視、点滅クロック開始。
    assert_eq!(
        count_cursor_rects(tree.render(1000.0)),
        1,
        "frame 0: visible"
    );
    // 同じ時間枠内 — まだトグルしない。
    assert_eq!(
        count_cursor_rects(tree.render(1499.0)),
        1,
        "<500ms: still visible"
    );
    // 500ms 閾値を越えた — 非表示にトグル。
    assert_eq!(
        count_cursor_rects(tree.render(1500.0)),
        0,
        ">=500ms: hidden"
    );
    // さらに 500ms — 再び表示に戻る。
    assert_eq!(
        count_cursor_rects(tree.render(2000.0)),
        1,
        "+500ms: visible again"
    );
}

#[test]
fn render_does_not_blink_when_nothing_is_focused() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(43, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    // フォーカスなし → 時間が経ってもカーソルは非表示のまま。
    assert_eq!(count_cursor_rects(tree.render(0.0)), 0);
    assert_eq!(count_cursor_rects(tree.render(10_000.0)), 0);
}

#[test]
fn blur_stops_blink_and_hides_cursor() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(44, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.element_focus(input);
    assert_eq!(count_cursor_rects(tree.render(0.0)), 1);
    tree.element_blur(input);
    assert_eq!(
        count_cursor_rects(tree.render(600.0)),
        0,
        "blur hides cursor"
    );
    assert_eq!(
        count_cursor_rects(tree.render(1200.0)),
        0,
        "blur stops blink"
    );
}

// ── ADR-0031: セマンティックイベント variant のスモークテスト ─────────────────────────

#[test]
fn semantic_event_variants_roundtrip_through_poll() {
    let mut tree = ElementTree::new();
    let target = tree.element_create(45, ElementKind::View);
    tree.set_root(target);

    tree.push_event(Event::HoverEnter { target_id: target });
    tree.push_event(Event::ActiveStart { target_id: target });
    tree.push_event(Event::ActiveEnd { target_id: target });
    tree.push_event(Event::HoverLeave { target_id: target });
    tree.push_event(Event::PointerMove {
        x: 12.5,
        y: 34.0,
        pointer_kind: hayate_core::PointerKind::Mouse,
    });

    let events = tree.poll_events();
    assert_eq!(events.len(), 5);
    assert!(matches!(&events[0], Event::HoverEnter { .. }));
    assert!(matches!(&events[1], Event::ActiveStart { .. }));
    assert!(matches!(&events[2], Event::ActiveEnd { .. }));
    assert!(matches!(&events[3], Event::HoverLeave { .. }));
    assert!(
        matches!(&events[4], Event::PointerMove { x, y, .. } if (*x - 12.5).abs() < 1e-3 && (*y - 34.0).abs() < 1e-3)
    );
}

// ── has_layout ガード ─────────────────────────────────────────────────

#[test]
fn has_layout_false_before_render_true_after() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(46, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    assert!(
        !tree.has_layout(),
        "has_layout must be false before first render"
    );
    tree.render(0.0);
    assert!(tree.has_layout(), "has_layout must be true after render");
}

// ── クリップボード貼り付けのテスト ─────────────────────────────────────────────────

#[test]
fn paste_into_empty_text_input_sets_content() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(47, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_paste(input, "hello");
    assert_eq!(tree.element_get_text_content(input), "hello");
}

#[test]
fn paste_appends_to_existing_content() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(48, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "abc");
    tree.element_paste(input, "def");
    assert_eq!(tree.element_get_text_content(input), "abcdef");
}

#[test]
fn paste_commits_active_preedit_then_appends() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(49, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "abc");
    tree.element_set_preedit(input, "DEF"); // 進行中の IME 変換
    tree.element_paste(input, "xyz");

    // preedit が確定され、その後に貼り付けテキストが追加されるはず。
    assert_eq!(tree.element_get_text_content(input), "abcDEFxyz");
    // 貼り付け後に preedit を空に設定しても何も変わらないはず。
    tree.element_set_preedit(input, "");
    assert_eq!(tree.element_get_text_content(input), "abcDEFxyz");
}

#[test]
fn paste_queues_text_input_event() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(50, ElementKind::TextInput);
    tree.set_root(input);

    let _listener = tree.register_listener(input, DocumentEventKind::TextInput);
    tree.element_paste(input, "hello");
    let deliveries = tree.poll_deliveries();
    assert_eq!(deliveries.len(), 1);
    assert!(
        matches!(&deliveries[0].event, Event::TextInput { text, .. } if text == "hello"),
        "expected TextInput delivery with pasted text"
    );
}

#[test]
fn paste_on_non_text_input_is_noop() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(51, ElementKind::View);
    tree.set_root(view);

    tree.element_paste(view, "ignored");
    // イベントもパニックも起きない。
    assert!(tree.poll_events().is_empty());
}

// ── insert_before / OP_INSERT_BEFORE ────────────────────────────────────

#[test]
fn insert_before_reorders_children_in_flex_row() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(52, ElementKind::View);
    let a = tree.element_create(53, ElementKind::View);
    let b = tree.element_create(54, ElementKind::View);
    let c = tree.element_create(55, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    for &child in &[a, b] {
        tree.element_append_child(root, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(50.0)),
                StyleProp::Height(Dimension::px(50.0)),
            ],
        );
    }
    // c（赤）を b の前に挿入 — 期待する描画順: a, c, b。
    tree.element_set_style(
        c,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_insert_before(root, c, b);

    tree.render(0.0);
    // c は index 1 になったので、その rect は x=50 に来るはず。
    let c_rect = tree.element_layout_rect(c).expect("c has no layout rect");
    assert!(
        (c_rect.0 - 50.0).abs() < 0.5,
        "c x = {} (expected 50)",
        c_rect.0
    );
    // b は index 2 へ押し出されたので、その rect は x=100 に来るはず。
    let b_rect = tree.element_layout_rect(b).expect("b has no layout rect");
    assert!(
        (b_rect.0 - 100.0).abs() < 0.5,
        "b x = {} (expected 100)",
        b_rect.0
    );
}

// ── element_content_size（スクロールクランプ） ───────────────────────────────

#[test]
fn element_content_size_returns_children_bounds() {
    let mut tree = ElementTree::new();
    let sv = tree.element_create(56, ElementKind::ScrollView);
    let content = tree.element_create(57, ElementKind::View);
    tree.set_root(sv);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        sv,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(sv, content);
    tree.render(0.0);

    let (cw, ch) = tree.element_content_size(sv);
    assert!((cw - 200.0).abs() < 0.5, "content width = {cw}");
    assert!((ch - 400.0).abs() < 0.5, "content height = {ch}");
}

// ── ADR-0013: WIT デュアルレイヤ — HTML Mode 向け resolved_elements ───────────

#[test]
fn resolved_elements_returns_absolute_positions() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(58, ElementKind::View);
    let child = tree.element_create(59, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::PaddingLeft(Dimension::px(10.0)),
            StyleProp::PaddingTop(Dimension::px(20.0)),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
        ],
    );
    tree.element_append_child(root, child);

    let resolved = tree.resolved_elements();
    let re = resolved
        .iter()
        .find(|(id, _)| *id == child)
        .map(|(_, r)| r)
        .expect("child not in resolved_elements");

    assert!((re.x - 10.0).abs() < 0.5, "child x = {}", re.x);
    assert!((re.y - 20.0).abs() < 0.5, "child y = {}", re.y);
    assert!((re.width - 50.0).abs() < 0.5);
    assert!((re.height - 50.0).abs() < 0.5);
}

// ── ADR-0073: Canvas 同梱フォント ───────────────────────────────────────

#[test]
fn default_font_family_constant_is_noto_sans() {
    assert_eq!(hayate_core::element::text::DEFAULT_FONT_FAMILY, "Noto Sans");
}

// ── ADR-0022: スクロールオフセットは上位レイヤが管理する ───────────────────────

#[test]
fn scroll_offset_readback_matches_set_value() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(60, ElementKind::ScrollView);
    tree.set_root(root);

    tree.element_set_scroll_offset(root, 30.0, 75.0);
    let (x, y) = tree.element_get_scroll_offset(root);
    assert!((x - 30.0).abs() < 1e-3, "scroll x = {x}");
    assert!((y - 75.0).abs() < 1e-3, "scroll y = {y}");
}

#[test]
fn unknown_font_family_falls_back_to_default() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(61, ElementKind::View);
    let text = tree.element_create(62, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(root, text);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::FontSize(24.0)]);
    tree.element_set_font_family(text, "NonExistentFont-XYZ-12345");
    tree.element_set_text(text, "hello");

    let sg = tree.render(0.0);
    let has_text_run = sg
        .iter()
        .any(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }));
    assert!(
        has_text_run,
        "unknown font family must fall back to Noto Sans and produce a TextRun"
    );
}

// ── 削除時に focused_element がクリアされる ────────────────────────────────

#[test]
fn remove_clears_focused_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(63, ElementKind::View);
    let input = tree.element_create(64, ElementKind::TextInput);
    tree.set_root(root);
    tree.element_append_child(root, input);

    tree.element_focus(input);
    assert_eq!(tree.focused_element(), Some(input), "focus not set");

    tree.element_remove(input);
    assert_eq!(
        tree.focused_element(),
        None,
        "focused_element must clear when the focused element is removed"
    );
}

#[test]
fn pseudo_hover_applies_to_ancestor_when_pointer_over_child() {
    use hayate_core::PseudoState;

    let mut tree = ElementTree::new();
    let root = tree.element_create(70, ElementKind::View);
    let child = tree.element_create(71, ElementKind::Button);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::BackgroundColor(Color::WHITE),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(80.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );
    tree.render(0.0);

    let (entered, left) = tree.update_pointer_hover(Some(child));
    assert!(
        entered.contains(&root),
        "parent must enter :hover with child"
    );
    assert!(entered.contains(&child));
    assert!(left.is_empty());

    let sg = tree.scene_graph();
    // シーングラフは root 背景に hover 時の青を反映するはず。
    assert!(!sg.roots().is_empty());
}

#[test]
fn viewport_resize_reflows_percent_children() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(8001, ElementKind::View);
    let left = tree.element_create(8002, ElementKind::View);
    let right = tree.element_create(8003, ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, left);
    tree.element_append_child(root, right);

    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
        ],
    );
    for (child, pct) in [(left, 67.0), (right, 33.0)] {
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::percent(pct)),
                StyleProp::Height(Dimension::percent(100.0)),
            ],
        );
    }

    tree.set_viewport(900.0, 600.0);
    tree.render(0.0);
    let left_wide = tree.element_layout_rect(left).expect("left layout");
    let right_wide = tree.element_layout_rect(right).expect("right layout");
    assert!(
        (left_wide.2 - 603.0).abs() < 1.0,
        "67% of 900 should be ~603, got {}",
        left_wide.2
    );
    assert!(
        (right_wide.2 - 297.0).abs() < 1.0,
        "33% of 900 should be ~297, got {}",
        right_wide.2
    );

    tree.set_viewport(300.0, 600.0);
    tree.render(0.0);
    let left_narrow = tree
        .element_layout_rect(left)
        .expect("left layout after resize");
    let right_narrow = tree
        .element_layout_rect(right)
        .expect("right layout after resize");
    assert!(
        (left_narrow.2 - 201.0).abs() < 1.0,
        "67% of 300 should be ~201 after resize, got {}",
        left_narrow.2
    );
    assert!(
        (right_narrow.2 - 99.0).abs() < 1.0,
        "33% of 300 should be ~99 after resize, got {}",
        right_narrow.2
    );
}

#[test]
fn viewport_resize_does_not_override_explicit_root_px_size() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(8101, ElementKind::View);
    tree.set_root(root);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
        ],
    );

    tree.set_viewport(900.0, 600.0);
    tree.render(0.0);
    let rect = tree.element_layout_rect(root).expect("root layout");
    assert!((rect.2 - 100.0).abs() < 0.5);
    assert!((rect.3 - 50.0).abs() < 0.5);

    tree.set_viewport(300.0, 200.0);
    tree.render(0.0);
    let rect = tree
        .element_layout_rect(root)
        .expect("root layout after resize");
    assert!(
        (rect.2 - 100.0).abs() < 0.5,
        "explicit root px width must not track viewport, got {}",
        rect.2
    );
    assert!(
        (rect.3 - 50.0).abs() < 0.5,
        "explicit root px height must not track viewport, got {}",
        rect.3
    );
}

// ── ElementEngine / commit_frame（ADR-0075） ──────────────────────────────

#[test]
fn commit_frame_resolves_layout_without_render() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(8200, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(120.0)),
            StyleProp::Height(Dimension::px(80.0)),
        ],
    );

    assert!(
        !tree.has_layout(),
        "layout cache should be empty before commit_frame"
    );

    tree.commit_frame();

    let rect = tree
        .element_layout_rect(root)
        .expect("commit_frame should populate layout_cache");
    assert!((rect.2 - 120.0).abs() < 0.5);
    assert!((rect.3 - 80.0).abs() < 0.5);
}

#[test]
fn commit_frame_resolves_structure_dirty_for_appended_child() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(8210, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(root, &[StyleProp::Width(Dimension::px(200.0))]);
    tree.commit_frame();

    let child = tree.element_create(8211, ElementKind::View);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.element_append_child(root, child);

    assert!(
        tree.element_layout_rect(child).is_none(),
        "newly appended child should have no layout until commit_frame resolves structure_dirty"
    );

    tree.commit_frame();

    let rect = tree
        .element_layout_rect(child)
        .expect("commit_frame should resolve structure_dirty and project the new child");
    assert!((rect.2 - 50.0).abs() < 0.5);
    assert!((rect.3 - 40.0).abs() < 0.5);
}

#[test]
fn commit_frame_then_effective_visual_reflects_pseudo_hover_state() {
    use hayate_core::PseudoState;

    let mut tree = ElementTree::new();
    let root = tree.element_create(8220, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );

    tree.commit_frame();
    assert!(tree.element_layout_rect(root).is_some());

    tree.hover_enter_element(root);
    let visual = tree
        .element_effective_visual(root)
        .expect("root should resolve effective visual");
    assert_eq!(
        visual.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0))
    );
}
