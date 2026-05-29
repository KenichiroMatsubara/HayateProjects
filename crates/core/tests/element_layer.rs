use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementKind, ElementTree, Event,
    FlexDirectionValue, NodeKind, StyleProp,
};

// ── Layout/rendering helpers ─────────────────────────────────────────────

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
    let root = tree.element_create(12, ElementKind::ScrollView);
    let content = tree.element_create(13, ElementKind::View);
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
    let sg = tree.render(0.0);

    let clip_count = sg.iter().filter(|(_, n)| matches!(n.kind, NodeKind::Clip { .. })).count();
    assert_eq!(clip_count, 1, "ScrollView should emit exactly one Clip node");
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
    let sg = tree.render(0.0);

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
    // Identity transform — Group node should appear, Rect should be its child.
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
    let root = tree.element_create(17, ElementKind::View);
    let back = tree.element_create(18, ElementKind::View);
    let front = tree.element_create(19, ElementKind::View);
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
    let sg = tree.render(0.0);
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
    let root = tree.element_create(20, ElementKind::View);
    let child = tree.element_create(21, ElementKind::View);
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
    tree.render(0.0);

    // Point inside child → child wins (deepest)
    assert_eq!(tree.hit_test(50.0, 50.0), Some(child));
    // Point outside child but inside root → root
    assert_eq!(tree.hit_test(200.0, 200.0), Some(root));
    // Point outside everything → None
    assert_eq!(tree.hit_test(500.0, 500.0), None);
}

#[test]
fn hit_test_respects_z_index_when_paint_and_doc_order_diverge() {
    // Regression: hit_test walked children in reverse document order, ignoring
    // z-index. scene_build sorts by z-index for paint order, so the visually
    // topmost element was unhittable when document order and z-index disagreed.
    let mut tree = ElementTree::new();
    let root = tree.element_create(22, ElementKind::View);
    let back = tree.element_create(23, ElementKind::View);
    let front = tree.element_create(24, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(200.0))],
    );
    // Flex column with a negative margin so the two 100×100 siblings overlap.
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    // `back` is appended FIRST but has z_index=1 (intended on top).
    // `front` is appended SECOND with z_index=0 (intended behind), but a
    // -100px margin-top makes it overlap `back` exactly.
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

    // Confirm the two children actually overlap at (50, 50).
    let back_rect = tree.element_layout_rect(back).unwrap();
    let front_rect = tree.element_layout_rect(front).unwrap();
    assert!(
        (back_rect.0 - front_rect.0).abs() < 0.5
            && (back_rect.1 - front_rect.1).abs() < 0.5,
        "test setup failed: children must overlap (back={back_rect:?}, front={front_rect:?})"
    );

    // Visually `back` is on top (z=1 > z=0), so hit_test must return `back`.
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
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(200.0))],
    );
    tree.render(0.0);

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
    let root = tree.element_create(26, ElementKind::ScrollView);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(300.0)), StyleProp::Height(Dimension::px(300.0))],
    );
    tree.render(0.0);

    let target = tree.hit_test(100.0, 100.0).expect("no hit");
    tree.push_event(Event::Scroll { target, delta_x: 0.0, delta_y: 20.0 });

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
    // After removing `a`, both `a` and `b` should be gone, but root remains.
    assert_eq!(tree.element_kind(root), Some(ElementKind::View));
    assert_eq!(tree.element_kind(a), None);
    assert_eq!(tree.element_kind(b), None);
}

// ── Phase 5: TextInput + IME tests ──────────────────────────────────────

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

    // Display text includes preedit suffix.
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
}

#[test]
fn preedit_renders_when_text_content_is_empty() {
    // Regression: typing IME composition into an empty TextInput must surface
    // the preedit as a TextRun. scene_build previously gated content rendering
    // on text_content alone, hiding the preedit until the user committed.
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

    // No committed text yet — only an in-progress IME composition.
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

    // After commit, preedit is part of committed text.
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
    // Setting preedit to empty effectively clears it.
    tree.element_set_preedit(input, "");
    assert_eq!(tree.element_get_text_content(input), "abcDEF");
}

#[test]
fn text_input_event_queued_on_append() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(35, ElementKind::TextInput);
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
    let input = tree.element_create(36, ElementKind::TextInput);
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

// ── Keyboard event tests (Enter key, modifiers) ──────────────────────────

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

    // Shift+A with modifier bitmask
    tree.push_event(Event::KeyDown { target: input, key: "A".to_string(), modifiers: 1 });
    let events = tree.poll_events();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0], Event::KeyDown { key, modifiers, .. } if key == "A" && *modifiers == 1)
    );
}

// ── Cursor visibility tests ───────────────────────────────────────────────

#[test]
fn cursor_visible_on_focus_hidden_on_blur() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(41, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_set_cursor_visible(input, true);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(input, &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(40.0))]);

    let sg = tree.render(0.0);
    // When cursor is visible and text_content is empty, a fallback Rect cursor is emitted.
    let cursor_rects: Vec<_> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, .. } if (*width - 1.5).abs() < 0.1 => Some(n),
            _ => None,
        })
        .collect();
    assert!(!cursor_rects.is_empty(), "cursor rect should be emitted when cursor_visible=true");

    tree.element_set_cursor_visible(input, false);
    let sg = tree.render(0.0);
    let cursor_rects: Vec<_> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, .. } if (*width - 1.5).abs() < 0.1 => Some(n),
            _ => None,
        })
        .collect();
    assert!(cursor_rects.is_empty(), "cursor rect should not be emitted when cursor_visible=false");
}

// ── ADR-0032: render(timestamp_ms) drives cursor blink internally ────────

fn count_cursor_rects(sg: &hayate_core::SceneGraph) -> usize {
    sg.iter()
        .filter(|(_, n)| matches!(&n.kind, NodeKind::Rect { width, .. } if (*width - 1.5).abs() < 0.1))
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
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(40.0))],
    );

    tree.element_focus(input);
    // First frame: cursor visible, blink clock starts.
    assert_eq!(count_cursor_rects(tree.render(1000.0)), 1, "frame 0: visible");
    // Same frame budget — no toggle yet.
    assert_eq!(count_cursor_rects(tree.render(1499.0)), 1, "<500ms: still visible");
    // Crossed 500ms threshold — toggle to hidden.
    assert_eq!(count_cursor_rects(tree.render(1500.0)), 0, ">=500ms: hidden");
    // Another 500ms — back to visible.
    assert_eq!(count_cursor_rects(tree.render(2000.0)), 1, "+500ms: visible again");
}

#[test]
fn render_does_not_blink_when_nothing_is_focused() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(43, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        input,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(40.0))],
    );
    // No focus → cursor stays hidden no matter how much time passes.
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
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(40.0))],
    );
    tree.element_focus(input);
    assert_eq!(count_cursor_rects(tree.render(0.0)), 1);
    tree.element_blur(input);
    assert_eq!(count_cursor_rects(tree.render(600.0)), 0, "blur hides cursor");
    assert_eq!(count_cursor_rects(tree.render(1200.0)), 0, "blur stops blink");
}

// ── ADR-0031: semantic event variant smoke tests ─────────────────────────

#[test]
fn semantic_event_variants_roundtrip_through_poll() {
    let mut tree = ElementTree::new();
    let target = tree.element_create(45, ElementKind::View);
    tree.set_root(target);

    tree.push_event(Event::HoverEnter { target });
    tree.push_event(Event::ActiveStart { target });
    tree.push_event(Event::ActiveEnd { target });
    tree.push_event(Event::HoverLeave { target });
    tree.push_event(Event::PointerMove { x: 12.5, y: 34.0 });

    let events = tree.poll_events();
    assert_eq!(events.len(), 5);
    assert!(matches!(&events[0], Event::HoverEnter { .. }));
    assert!(matches!(&events[1], Event::ActiveStart { .. }));
    assert!(matches!(&events[2], Event::ActiveEnd { .. }));
    assert!(matches!(&events[3], Event::HoverLeave { .. }));
    assert!(matches!(&events[4], Event::PointerMove { x, y } if (*x - 12.5).abs() < 1e-3 && (*y - 34.0).abs() < 1e-3));
}

// ── T4: has_layout guard ─────────────────────────────────────────────────

#[test]
fn has_layout_false_before_render_true_after() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(46, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(200.0))],
    );
    assert!(!tree.has_layout(), "has_layout must be false before first render");
    tree.render(0.0);
    assert!(tree.has_layout(), "has_layout must be true after render");
}

// ── Clipboard paste tests ─────────────────────────────────────────────────

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
    tree.element_set_preedit(input, "DEF"); // in-progress IME composition
    tree.element_paste(input, "xyz");

    // Preedit should be committed and pasted text appended after it.
    assert_eq!(tree.element_get_text_content(input), "abcDEFxyz");
    // Setting preedit to empty after paste should not change anything.
    tree.element_set_preedit(input, "");
    assert_eq!(tree.element_get_text_content(input), "abcDEFxyz");
}

#[test]
fn paste_queues_text_input_event() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(50, ElementKind::TextInput);
    tree.set_root(input);

    tree.element_paste(input, "hello");
    let events = tree.poll_events();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0], Event::TextInput { text, .. } if text == "hello"),
        "expected TextInput event with pasted text"
    );
}

#[test]
fn paste_on_non_text_input_is_noop() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(51, ElementKind::View);
    tree.set_root(view);

    tree.element_paste(view, "ignored");
    // No events, no panic.
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
            &[StyleProp::Width(Dimension::px(50.0)), StyleProp::Height(Dimension::px(50.0))],
        );
    }
    // Insert c (red) before b — expected paint order: a, c, b.
    tree.element_set_style(
        c,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_insert_before(root, c, b);

    let sg = tree.render(0.0);
    // c is now at index 1, so its rect should sit at x=50.
    let c_rect = tree.element_layout_rect(c).expect("c has no layout rect");
    assert!((c_rect.0 - 50.0).abs() < 0.5, "c x = {} (expected 50)", c_rect.0);
    // b is pushed to index 2, so its rect should sit at x=100.
    let b_rect = tree.element_layout_rect(b).expect("b has no layout rect");
    assert!((b_rect.0 - 100.0).abs() < 0.5, "b x = {} (expected 100)", b_rect.0);
}

// ── element_content_size (scroll clamping) ───────────────────────────────

#[test]
fn element_content_size_returns_children_bounds() {
    let mut tree = ElementTree::new();
    let sv = tree.element_create(56, ElementKind::ScrollView);
    let content = tree.element_create(57, ElementKind::View);
    tree.set_root(sv);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        sv,
        &[StyleProp::Width(Dimension::px(200.0)), StyleProp::Height(Dimension::px(100.0))],
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

// ── ADR-0013: WIT Dual Layer — resolved_elements for HTML Mode ───────────

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
        &[StyleProp::Width(Dimension::px(50.0)), StyleProp::Height(Dimension::px(50.0))],
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

// ── ADR-0028: Canvas bundled fonts ───────────────────────────────────────

#[test]
fn default_font_family_constant_is_noto_sans() {
    assert_eq!(hayate_core::element::text::DEFAULT_FONT_FAMILY, "Noto Sans");
}

// ── ADR-0022: scroll offset managed by upper layer ───────────────────────

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
        &[StyleProp::Width(Dimension::px(400.0)), StyleProp::Height(Dimension::px(300.0))],
    );
    tree.element_set_style(text, &[StyleProp::FontSize(24.0)]);
    tree.element_set_font_family(text, "NonExistentFont-XYZ-12345");
    tree.element_set_text(text, "hello");

    let sg = tree.render(0.0);
    let has_text_run = sg.iter().any(|(_, n)| matches!(&n.kind, NodeKind::TextRun { .. }));
    assert!(has_text_run, "unknown font family must fall back to Noto Sans and produce a TextRun");
}

// ── T3: focused_element cleared on remove ────────────────────────────────

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
