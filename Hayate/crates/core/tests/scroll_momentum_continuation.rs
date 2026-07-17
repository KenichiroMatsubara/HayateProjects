//! リグレッション（ADR-0126 の on-demand フレームループが慣性を殺した回帰, #608–#615）。
//!
//! リリース済み慣性スクロールは Core が所有し（`scroll` モジュールの純物理を Core の
//! `render` が毎フレーム積分する）、進行中は `has_pending_visual_work` を true に保つ。
//! これにより on-demand ループ（進行中 visual work のあるフレームだけ produce）は、指を
//! 離した直後に idle へ落ちて慣性を 1 フレームで殺さず、静止まで自走する。慣性が Platform
//! Adapter 側にあった頃は Core の継続シグナルから見えず、まさにこれが壊れていた。

use hayate_core::scroll::PointerKind;
use hayate_core::{Dimension, ElementKind, ElementTree, StyleProp};

/// 縦 500px のコンテンツを高さ 100px の ScrollView に入れた、縦スクロール可能なツリー。
/// レイアウトを確定させ（`element_scroll_max_offset > 0` を成立させ）、`last_frame_ms` を
/// シードするため 1 度 render 済みにして返す。
fn scrollable() -> (ElementTree, hayate_core::ElementId) {
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
        ],
    );
    tree.element_append_child(scroll, content);
    tree.render(0.0);
    (tree, scroll)
}

#[test]
fn a_released_fling_keeps_pending_visual_work_until_it_settles() {
    let (mut tree, scroll) = scrollable();

    // 下向きフリックを起動（オフセット空間で正の vy = コンテンツが上へ流れる）。
    tree.start_scroll_momentum(scroll, 0.0, 2.0);
    assert!(
        tree.has_pending_visual_work(),
        "慣性起動直後は継続フレームを要求しなければならない（idle に落ちない）",
    );

    // render を回し続けると慣性が減衰し、やがて静止して idle に落ちる。これは
    // on-demand ループの継続条件そのもの——has_pending_visual_work が true の間だけ回す。
    let mut frames = 0;
    let mut t = 16.0;
    while tree.has_pending_visual_work() {
        tree.render(t);
        t += 16.0;
        frames += 1;
        assert!(
            frames < 2000,
            "慣性は有限フレームで静止しなければならない（無限に這わない）"
        );
    }

    assert!(
        frames > 5,
        "摩擦減衰は複数フレームにわたって滑走する——1 フレームで死んではならない（回帰の芯, frames={frames})",
    );
    let (_, oy) = tree.element_get_scroll_offset(scroll);
    assert!(oy > 0.0, "慣性でコンテンツが実際に動いた（got {oy}）");
}

#[test]
fn a_new_pointer_down_cancels_momentum() {
    let (mut tree, scroll) = scrollable();
    tree.start_scroll_momentum(scroll, 0.0, 2.0);
    assert!(tree.has_pending_visual_work());
    let (_, oy_before) = tree.element_get_scroll_offset(scroll);

    // 新規押下は惰性を中断し、コンテンツを即座に掴めるようにする（ADR-0082）。
    // 押下自体は :active 等の visual work を生むので has_pending は true になり得るが、
    // 慣性はもう offset を動かしてはならない——それが「掴んで止まる」挙動。
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    for i in 1..=8 {
        tree.render(16.0 * (i as f64 + 1.0));
    }
    let (_, oy_after) = tree.element_get_scroll_offset(scroll);
    assert_eq!(
        oy_before, oy_after,
        "キャンセルされた慣性は以後 1px も滑ってはならない（掴んだら即停止）",
    );
}

#[test]
fn a_slow_in_range_release_does_not_animate() {
    let (mut tree, scroll) = scrollable();
    // 範囲内・ほぼ静止で離す（min_velocity 未満）：フリックもばね戻しも起こらない。
    tree.start_scroll_momentum(scroll, 0.0, 0.0);
    assert!(
        !tree.has_pending_visual_work(),
        "範囲内で止めて離したらアニメーションしない（無駄フレームを出さない）",
    );
}
