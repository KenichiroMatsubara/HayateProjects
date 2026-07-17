//! ScrollView のシーングラフ lowering: 子アンカーは Clip/scroll ラッパー配下に置かれねばならない。

use hayate_core::{
    Color, Dimension, ElementKind, ElementTree, NodeKind, ScrollPhysicsProfile,
    ScrollPhysicsTuning, StyleProp,
};

fn has_ancestor_matching(
    tree: &ElementTree,
    node: hayate_core::NodeId,
    pred: impl Fn(&NodeKind) -> bool,
) -> bool {
    let sg = tree.scene_graph();
    let mut current = Some(node);
    while let Some(id) = current {
        let Some(n) = sg.get(id) else {
            break;
        };
        if pred(&n.kind) {
            return true;
        }
        current = sg.parent_of(id);
    }
    false
}

fn scrolled_scroll_view_tree() -> (ElementTree, hayate_core::ElementId, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        scroll,
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
    tree.element_append_child(scroll, content);
    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    (tree, scroll, content)
}

/// content anchor から祖先を辿り、最初の scroll `Group` の 2x3 アフィンを返す。
fn scroll_group_transform(tree: &ElementTree, content: hayate_core::ElementId) -> [f64; 6] {
    let sg = tree.scene_graph();
    let mut current = Some(tree.test_element_anchor_id(content));
    while let Some(id) = current {
        let Some(n) = sg.get(id) else { break };
        if let NodeKind::Group { transform } = n.kind {
            return transform;
        }
        current = sg.parent_of(id);
    }
    panic!("no scroll Group ancestor found for content anchor");
}

#[test]
fn ios_profile_overscroll_translates_content_without_scaling() {
    // 既定（Auto → iOS）は overshoot 込みで content を丸ごと translate、scale 無し。回帰固定。
    let (mut tree, scroll, content) = scrolled_scroll_view_tree();
    tree.render(0.0);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    // 下端を 20px 越えて overscroll（未クランプ・SCR-02）。
    tree.element_set_scroll_offset(scroll, 0.0, max_y + 20.0);
    tree.render(0.0);

    let m = scroll_group_transform(&tree, content);
    assert_eq!(
        m,
        [1.0, 0.0, 0.0, 1.0, 0.0, -(max_y as f64) - 20.0],
        "iOS overscroll = full translate, no scale",
    );
}

#[test]
fn android_profile_overscroll_pins_edge_and_scales_content() {
    // profile: android は overshoot を「端クランプ translate ＋ ピン端アンカー scale」へ分割する。
    let (mut tree, scroll, content) = scrolled_scroll_view_tree();
    tree.set_scroll_profile(ScrollPhysicsProfile::Android);
    tree.render(0.0);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    // 下端を 20px 越えて overscroll。ビューポート（scroll view）高さ = 100。
    tree.element_set_scroll_offset(scroll, 0.0, max_y + 20.0);
    tree.render(0.0);

    let m = scroll_group_transform(&tree, content);
    // 縦は伸びる: scale_y = 1 + (20/100)*STRETCH_MAX。
    let want_scale = 1.0 + (20.0 / 100.0) * ScrollPhysicsTuning::default().stretch_max as f64;
    assert!(
        (m[3] - want_scale).abs() < 1e-4,
        "y scale = {} (want {want_scale})",
        m[3]
    );
    // 横はスクロール不可（content 幅 == view 幅）→ 伸びない（軸独立）。
    assert_eq!(m[0], 1.0, "x is not stretched");
    assert_eq!(m[4], 0.0, "x translate stays 0");
    // ピン留めした下端はビューポート下端（シーン y=100）に固定されたまま。範囲内 max では
    // content-bottom は content-y = 100 + max_y（100 + max_y − max_y = 100）。
    let pinned = m[3] * (100.0 + max_y as f64) + m[5];
    assert!(
        (pinned - 100.0).abs() < 1e-3,
        "bottom edge pinned at viewport bottom (got {pinned})"
    );
    // scale が入っている以上、恒等 translate ではない。
    assert!(m[3] > 1.0, "content is stretched past the edge");
}

#[test]
fn scroll_view_child_anchor_is_descendant_of_clip_and_scroll_group() {
    let (mut tree, _scroll, content) = scrolled_scroll_view_tree();
    tree.render(0.0);

    let child_anchor = tree.test_element_anchor_id(content);
    assert!(
        has_ancestor_matching(&tree, child_anchor, |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "child ElementAnchor must be nested under the ScrollView Clip node"
    );
    assert!(
        has_ancestor_matching(&tree, child_anchor, |kind| {
            matches!(kind, NodeKind::Group { .. })
        }),
        "scrolled ScrollView child must be nested under the scroll-offset Group"
    );
}

#[test]
fn scroll_view_relowering_preserves_child_under_clip_wrapper() {
    let (mut tree, scroll, content) = scrolled_scroll_view_tree();
    tree.render(0.0);
    let content_anchor_after_first = tree.test_element_anchor_id(content);
    assert!(
        has_ancestor_matching(&tree, content_anchor_after_first, |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "initial build must nest content under Clip"
    );

    tree.element_set_scroll_offset(scroll, 0.0, 120.0);
    tree.render(0.0);

    let content_anchor_after_scroll = tree.test_element_anchor_id(content);
    assert_eq!(
        content_anchor_after_scroll, content_anchor_after_first,
        "content anchor must stay stable across re-lowering"
    );
    assert!(
        has_ancestor_matching(&tree, content_anchor_after_scroll, |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "re-lowering after scroll change must keep content under Clip"
    );
    assert!(
        has_ancestor_matching(&tree, content_anchor_after_scroll, |kind| {
            matches!(kind, NodeKind::Group { .. })
        }),
        "re-lowering after scroll change must keep content under scroll-offset Group"
    );

    tree.element_set_style(
        content,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    assert_eq!(
        tree.test_element_anchor_id(content),
        content_anchor_after_first,
        "content anchor must survive child visual re-lowering"
    );
    assert!(
        has_ancestor_matching(&tree, tree.test_element_anchor_id(content), |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "child visual change must not detach content from Clip wrapper chain"
    );
}
