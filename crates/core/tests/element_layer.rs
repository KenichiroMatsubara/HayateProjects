use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, Event,
    FlexDirectionValue, NodeKind, StyleProp,
};

#[test]
fn element_create_and_append_builds_tree() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let child_a = tree.element_create(ElementKind::View);
    let child_b = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, child_a);
    tree.element_append_child(root, child_b);
    assert_eq!(tree.root(), Some(root));
}

#[test]
fn set_style_routes_layout_and_visual() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(ElementKind::View);
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
    let sg = tree.render();
    // Expect a single Rect node with the layout-computed size.
    let mut found = false;
    for (_, n) in sg.iter() {
        if let NodeKind::Rect { width, height, color, .. } = &n.kind {
            assert!((*width - 100.0).abs() < 0.5);
            assert!((*height - 50.0).abs() < 0.5);
            assert!((color[0] - 1.0).abs() < 1e-3);
            found = true;
        }
    }
    assert!(found, "background rect not emitted");
}

#[test]
fn flex_row_positions_children_with_gap() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let a = tree.element_create(ElementKind::View);
    let b = tree.element_create(ElementKind::View);
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

    let sg = tree.render();
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
fn text_element_produces_text_run() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let text = tree.element_create(ElementKind::Text);
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
    let sg = tree.render();
    let has_text_run = sg
        .iter()
        .any(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }));
    assert!(has_text_run, "no TextRun emitted for text element");
}

#[test]
fn scene_build_walks_absolute_coordinates() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let child = tree.element_create(ElementKind::View);
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
    let sg = tree.render();
    let mut child_pos = None;
    for (_, n) in sg.iter() {
        if let NodeKind::Rect { x, y, width, height, color, .. } = &n.kind {
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

// ── ScrollView tests ─────────────────────────────────────────────────────

#[test]
fn scroll_view_emits_clip_node() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::ScrollView);
    let content = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(100.0))],
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
    let sg = tree.render();

    let clip_count = sg.iter().filter(|(_, n)| matches!(n.kind, NodeKind::Clip { .. })).count();
    assert_eq!(clip_count, 1, "ScrollView should emit exactly one Clip node");
}

#[test]
fn scroll_view_clip_contains_content_as_child() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::ScrollView);
    let content = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(100.0))],
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

    // Apply a scroll offset and verify it produces a Group inside the Clip.
    tree.element_set_scroll_offset(root, 0.0, 50.0);
    let sg = tree.render();

    // Find the Clip node — it should be in the roots list.
    let clip_root = sg
        .roots()
        .iter()
        .find(|&&id| matches!(sg.get(id).unwrap().kind, NodeKind::Clip { .. }))
        .copied()
        .expect("Clip should be a root node");
    let clip_node = sg.get(clip_root).unwrap();
    // Clip's first child should be a Group (scroll translate).
    assert!(!clip_node.children.is_empty(), "Clip should have children");
    let first_child = sg.get(clip_node.children[0]).unwrap();
    assert!(
        matches!(first_child.kind, NodeKind::Group { .. }),
        "Clip's first child should be a Group (scroll offset)"
    );
}

// ── Transform / Group tests ──────────────────────────────────────────────

#[test]
fn transform_emits_group_node() {
    use hayate_core::{NodeKind};
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
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
    // Identity transform — Group node should appear, Rect should be its child.
    let identity = [1.0_f64, 0.0, 0.0, 1.0, 0.0, 0.0];
    tree.element_set_transform(root, Some(identity));
    let sg = tree.render();

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

    // Group should be a root; Rect should be inside the Group (a child, not a root).
    let groups: Vec<_> = sg
        .roots()
        .iter()
        .filter(|&&id| matches!(sg.get(id).unwrap().kind, NodeKind::Group { .. }))
        .copied()
        .collect();
    assert_eq!(groups.len(), 1, "Group should be a root node");
    let group_node = sg.get(groups[0]).unwrap();
    assert_eq!(group_node.children.len(), 1, "Rect should be a child of Group");
}

// ── ZIndex tests ─────────────────────────────────────────────────────────

#[test]
fn z_index_controls_paint_order() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let back = tree.element_create(ElementKind::View);
    let front = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(200.0))],
    );
    // back is appended first but gets z_index 1; front appended second but z_index 0.
    // After sort, front (z=0) should be painted before back (z=1).
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
    let sg = tree.render();
    // Collect paint order: look at first-component of color for each 50×50 rect.
    let order: Vec<f32> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, color, .. } if (*width - 50.0).abs() < 0.5 => Some(color[0]),
            _ => None,
        })
        .collect();
    // front (blue, r=0) should come before back (red, r=1).
    assert_eq!(order.len(), 2, "expected 2 child rects");
    assert!((order[0] - 0.0).abs() < 1e-3, "front (blue) first");
    assert!((order[1] - 1.0).abs() < 1e-3, "back (red) second");
}

// ── Event system tests ───────────────────────────────────────────────────

#[test]
fn hit_test_returns_deepest_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let child = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 400.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(400.0)), StyleProp::Height(Dimension::px(400.0))],
    );
    tree.element_set_style(
        child,
        &[StyleProp::Width(Dimension::px(100.0)), StyleProp::Height(Dimension::px(100.0))],
    );
    tree.element_append_child(root, child);
    tree.render();

    // Point inside child → child wins (deepest)
    assert_eq!(tree.hit_test(50.0, 50.0), Some(child));
    // Point outside child but inside root → root
    assert_eq!(tree.hit_test(200.0, 200.0), Some(root));
    // Point outside everything → None
    assert_eq!(tree.hit_test(500.0, 500.0), None);
}

#[test]
fn push_and_poll_events() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(200.0))],
    );
    tree.render();

    tree.push_event(Event::Click { target: root, x: 10.0, y: 20.0 });
    tree.push_event(Event::Resize { width: 300.0, height: 400.0 });

    let events = tree.poll_events();
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0], Event::Click { x, .. } if (*x - 10.0).abs() < 1e-3));
    assert!(matches!(&events[1], Event::Resize { width, .. } if (*width - 300.0).abs() < 1e-3));

    // Queue is drained after poll.
    assert!(tree.poll_events().is_empty());
}

#[test]
fn scroll_event_targets_hit_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::ScrollView);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(300.0)), StyleProp::Height(Dimension::px(300.0))],
    );
    tree.render();

    let target = tree.hit_test(100.0, 100.0).expect("no hit");
    tree.push_event(Event::Scroll { target, delta_x: 0.0, delta_y: 20.0 });

    let events = tree.poll_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], Event::Scroll { delta_y, .. } if (*delta_y - 20.0).abs() < 1e-3));
}

#[test]
fn remove_subtree_drops_children() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(ElementKind::View);
    let a = tree.element_create(ElementKind::View);
    let b = tree.element_create(ElementKind::View);
    tree.set_root(root);
    tree.element_append_child(root, a);
    tree.element_append_child(a, b);
    tree.element_remove(a);
    // After removing `a`, both `a` and `b` should be gone, but root remains.
    assert_eq!(tree.element_kind(root), Some(ElementKind::View));
    assert_eq!(tree.element_kind(a), None);
    assert_eq!(tree.element_kind(b), None);
}

// ── Phase 5: TextInput + IME tests ──────────────────────────────────────

#[test]
fn text_input_append_and_get() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "hello");
    assert_eq!(tree.element_get_text_content(input), "hello");

    tree.element_append_text_content(input, " world");
    assert_eq!(tree.element_get_text_content(input), "hello world");
}

#[test]
fn text_input_set_replaces_content() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "old");
    tree.element_set_text_content(input, "new");
    assert_eq!(tree.element_get_text_content(input), "new");
}

#[test]
fn preedit_shown_inline_not_committed() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "abc");
    tree.element_set_preedit(input, "DEF");

    // Display text includes preedit suffix.
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
}

#[test]
fn commit_preedit_appends_and_clears() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "abc");
    tree.element_set_preedit(input, "DEF");
    tree.element_commit_preedit(input);

    // After commit, preedit is part of committed text.
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
    // Setting preedit to empty effectively clears it.
    tree.element_set_preedit(input, "");
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
}

#[test]
fn text_input_event_queued_on_append() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(ElementKind::TextInput);
    tree.set_root(input);

    tree.element_append_text_content(input, "x");
    tree.push_event(Event::TextInput { target: input, text: "x".to_string() });

    let events = tree.poll_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], Event::TextInput { text, .. } if text == "x"));
}

#[test]
fn composition_lifecycle_events_queued() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(ElementKind::TextInput);
    tree.set_root(input);

    tree.push_event(Event::CompositionStart { target: input, text: "あ".to_string() });
    tree.push_event(Event::CompositionUpdate { target: input, text: "あい".to_string() });
    tree.push_event(Event::CompositionEnd { target: input, text: "愛".to_string() });

    let events = tree.poll_events();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], Event::CompositionStart { text, .. } if text == "あ"));
    assert!(matches!(&events[1], Event::CompositionUpdate { text, .. } if text == "あい"));
    assert!(matches!(&events[2], Event::CompositionEnd { text, .. } if text == "愛"));
}
