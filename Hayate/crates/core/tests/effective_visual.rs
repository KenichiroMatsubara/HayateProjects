//! 共有 effective visual リゾルバとクエリ API（ADR-0067）。

use hayate_core::{
    BorderStyleValue, Color, Dimension, ElementKind, ElementTree, NodeKind, PseudoState, StyleProp,
    ViewportCondition,
};

#[test]
fn element_effective_visual_applies_hover_pseudo() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0))],
    );
    tree.element_set_pseudo_style(
        id,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );

    let base = tree.element_effective_visual(id).unwrap();
    assert_eq!(base.background_color, Some(Color::new(1.0, 1.0, 1.0, 1.0)));

    tree.update_pointer_hover(Some(id));
    let hovered = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        hovered.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        ":hover pseudo must apply via element_effective_visual"
    );
}

#[test]
fn element_effective_visual_defaults_border_style_to_none() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);

    let base = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        base.border_style,
        BorderStyleValue::None,
        "border-style must default to none (a border needs an explicit style)"
    );
}

#[test]
fn element_effective_visual_resolves_explicit_border_style() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.element_set_style(id, &[StyleProp::BorderStyle(BorderStyleValue::Dashed)]);

    let resolved = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        resolved.border_style,
        BorderStyleValue::Dashed,
        "explicit border-style must resolve through effective visual"
    );
}

#[test]
fn element_effective_visual_border_style_pseudo_override() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.element_set_style(id, &[StyleProp::BorderStyle(BorderStyleValue::Solid)]);
    tree.element_set_pseudo_style(
        id,
        PseudoState::Hover,
        &[StyleProp::BorderStyle(BorderStyleValue::Dashed)],
    );

    let base = tree.element_effective_visual(id).unwrap();
    assert_eq!(base.border_style, BorderStyleValue::Solid);

    tree.update_pointer_hover(Some(id));
    let hovered = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        hovered.border_style,
        BorderStyleValue::Dashed,
        ":hover border-style must override the base border-style"
    );
}

#[test]
fn element_effective_visual_viewport_condition_below_min_width_uses_base() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(500.0, 800.0);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );

    let visual = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        visual.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "viewport width below min-width must keep the base style"
    );
}

#[test]
fn element_effective_visual_viewport_condition_at_min_width_uses_variant() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(768.0, 800.0);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );

    let visual = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        visual.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "viewport width equal to min-width must apply the variant (inclusive)"
    );
}

#[test]
fn element_effective_visual_viewport_compound_and_condition() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            max_width: Some(1024.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );

    tree.set_viewport(900.0, 800.0);
    let inside = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        inside.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "viewport inside min-width and max-width range must apply the variant"
    );

    tree.set_viewport(1100.0, 800.0);
    let above_max = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        above_max.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "viewport above max-width must keep the base style"
    );
}

#[test]
fn element_effective_visual_viewport_variant_cascade_last_match_wins() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(1100.0, 800.0);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(1024.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
    );

    let visual = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        visual.background_color,
        Some(Color::new(0.0, 1.0, 0.0, 1.0)),
        "when multiple variants match, declaration order last match must win"
    );

    tree.set_viewport(900.0, 800.0);
    let single_match = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        single_match.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "only the first matching variant must apply when later variants do not match"
    );

    tree.set_viewport(500.0, 800.0);
    let no_match = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        no_match.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "when no variant matches, base style must remain"
    );
}

#[test]
fn element_effective_visual_viewport_height_axes() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_height: Some(600.0),
            max_height: Some(900.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );

    tree.set_viewport(1024.0, 700.0);
    let inside = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        inside.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "viewport height inside min-height and max-height range must apply the variant"
    );

    tree.set_viewport(1024.0, 500.0);
    let below_min = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        below_min.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "viewport height below min-height must keep the base style"
    );
}

#[test]
fn element_effective_visual_active_pseudo_wins_over_hover_when_both_match() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0))],
    );
    tree.element_set_pseudo_style(
        id,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.element_set_pseudo_style(
        id,
        PseudoState::Active,
        &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );

    tree.update_pointer_hover(Some(id));
    tree.on_pointer_down_on(id, 0.0, 0.0);

    let visual = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        visual.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        ":active pseudo must win over :hover (focus < hover < active)"
    );
}

#[test]
fn element_effective_visual_hover_pseudo_overrides_active_viewport_variant() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(1024.0, 800.0);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );
    tree.element_set_pseudo_style(
        id,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );

    let visual = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        visual.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "active viewport variant must apply when not hovered"
    );

    tree.update_pointer_hover(Some(id));
    let hovered = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        hovered.background_color,
        Some(Color::new(0.0, 1.0, 0.0, 1.0)),
        ":hover pseudo must override the active viewport variant"
    );
}

// クエリ経路（`element_effective_visual`）とシーン経路（`render`）は、ビューポート
// バリアント適用を含む単一の effective-visual 解決を共有する（ADR-0067, ADR-0081）。
// ビューポート条件付き要素について両者は一致しなければならない。
#[test]
fn query_and_scene_paths_share_viewport_variant_resolution() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.set_viewport(1024.0, 800.0);
    tree.element_set_style(
        id,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_style_variant(
        id,
        ViewportCondition {
            min_width: Some(768.0),
            ..Default::default()
        },
        StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
    );

    let queried = tree.element_effective_visual(id).unwrap().background_color;
    assert_eq!(
        queried,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "query path must resolve the active viewport variant"
    );

    tree.render(0.0);
    let scene_color = tree
        .scene_graph()
        .iter()
        .find_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, color, .. } if (*width - 200.0).abs() < 0.5 => Some(*color),
            _ => None,
        })
        .expect("root rect in scene");

    let q = queried.unwrap();
    assert!(
        (scene_color[0] as f64 - q.r).abs() < 1e-3
            && (scene_color[1] as f64 - q.g).abs() < 1e-3
            && (scene_color[2] as f64 - q.b).abs() < 1e-3,
        "scene path must resolve the same viewport variant as the query path \
         (single shared seam): query={q:?} scene={scene_color:?}"
    );
}

// 継承コンテキストの構築は、クエリ経路（`element_effective_visual` の祖先たどり）と
// シーン経路（`render` のトップダウン伝播）が共有する単一の fold。Text 要素が自身の
// ambient `default-color` を設定したとき、両経路は同じ継承コンテキストを構築し同じ
// テキスト色を解決しなければならない。`default-*` は子孫向けの ambient チャネル
// （ADR-0065）なので、要素自身の `default-color` が一方の経路でだけ自テキストを
// 着色するようなことがあってはならない。
#[test]
fn query_and_scene_paths_share_inherited_context_for_own_ambient_default() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(200.0, 100.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    // text 要素が自身の ambient default-color（子孫向けチャネル）を設定する。
    tree.element_set_style(text, &[StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0))]);
    tree.element_set_text(text, "hi");

    let queried = tree
        .element_effective_visual(text)
        .unwrap()
        .text_color
        .expect("query resolves a text color");

    tree.render(0.0);
    let scene_color = tree
        .scene_graph()
        .iter()
        .find_map(|(_, n)| match &n.kind {
            NodeKind::TextRun { color, .. } => Some(*color),
            _ => None,
        })
        .expect("text run in scene");

    assert!(
        (scene_color[0] as f64 - queried.r).abs() < 1e-3
            && (scene_color[1] as f64 - queried.g).abs() < 1e-3
            && (scene_color[2] as f64 - queried.b).abs() < 1e-3,
        "query and scene must build the inherited context through one shared fold: \
         query={queried:?} scene={scene_color:?}"
    );
}

#[test]
fn element_effective_visual_resolves_ambient_default_on_text() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(2, ElementKind::View);
    let text = tree.element_create(3, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(200.0, 100.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_text(text, "hi");

    let visual = tree.element_effective_visual(text).unwrap();
    assert_eq!(
        visual.text_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "ambient default-color must resolve on text element"
    );
}

#[test]
fn element_effective_visual_text_to_text_inheritance() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(4, ElementKind::View);
    let ifc = tree.element_create(5, ElementKind::Text);
    let inline = tree.element_create(6, ElementKind::Text);
    tree.set_root(view);
    tree.element_append_child(view, ifc);
    tree.element_append_child(ifc, inline);
    tree.element_set_style(ifc, &[StyleProp::FontSize(20.0)]);
    tree.element_set_text(ifc, "A");
    tree.element_set_text(inline, "B");

    let visual = tree.element_effective_visual(inline).unwrap();
    assert!(
        (visual.font_size.unwrap() - 20.0).abs() < 0.1,
        "inline text must inherit IFC root font-size"
    );
}

#[test]
fn view_font_size_does_not_leak_via_effective_visual() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(7, ElementKind::View);
    let text = tree.element_create(8, ElementKind::Text);
    tree.set_root(view);
    tree.element_append_child(view, text);
    tree.element_set_style(view, &[StyleProp::FontSize(24.0)]);
    tree.element_set_text(text, "x");

    let visual = tree.element_effective_visual(text).unwrap();
    assert!(
        (visual.font_size.unwrap() - 16.0).abs() < 0.1,
        "view font-size must not leak to child text"
    );
}
