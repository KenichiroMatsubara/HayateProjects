//! タッチドラッグ→スクロール配線（ADR-0082）。Web アダプタの参照実装
//! （`hayate-adapter-web` の `HayateElementRenderer`、`crates/platform/web/src/canvas.rs`）を
//! Android 向けに移植したもの。NDK 型を一切持たないので、実機なしでホスト
//! `cargo test` から振る舞いを固定できる（`app.rs::process_touch_input` が
//! 実 `MotionEvent` からこれを駆動する）。
//!
//! `hayate_core::scroll` の純物理・純判定（`ScrollGesture`/`MoveOutcome`/
//! `rubber_band_offset`/`estimate_release_velocity`）はここでは変更せず、Web と
//! 同じ配線パターンだけを適用する。

use hayate_core::scroll::{self, MoveOutcome, ScrollGesture, ScrollPhysicsTuning};
use hayate_core::{ElementId, ElementTree, PointerKind};

use crate::touch_input::PointerInput;

/// アクティブな touch ドラッグ→スクロールジェスチャと、そのフレーム間状態（ADR-0082）。
/// Android は単一ポインタのみ扱う（`app.rs::process_touch_input` 参照）ので、追跡は
/// 1 ジェスチャ分で足りる。
pub(crate) struct TouchScrollState {
    scroll_gesture: Option<ScrollGesture>,
    pending_pointer_down: Option<(f32, f32, f64)>,
    scroll_samples: Vec<(f32, f32, f64)>,
    drag_raw: Option<(ElementId, (f32, f32))>,
    tuning: ScrollPhysicsTuning,
}

impl TouchScrollState {
    pub(crate) fn new() -> Self {
        Self {
            scroll_gesture: None,
            pending_pointer_down: None,
            scroll_samples: Vec::new(),
            drag_raw: None,
            tuning: ScrollPhysicsTuning::default(),
        }
    }

    /// 1 件の [`PointerInput`] を `tree` に適用する。`now_ms` は解放速度推定用の
    /// 指サンプルへ刻むタイムスタンプ（呼び元の単調クロック）。
    pub(crate) fn apply(&mut self, tree: &mut ElementTree, input: PointerInput, now_ms: f64) {
        if !matches!(input, PointerInput::Down { .. }) {
            self.advance_press(tree, now_ms);
        }
        match input {
            PointerInput::Down { x, y } => self.on_down(tree, x, y, now_ms),
            PointerInput::Move { x, y } => self.on_move(tree, x, y, now_ms),
            PointerInput::Up { x, y } => self.on_up(tree, x, y),
            PointerInput::Cancel => self.on_cancel(tree),
        }
    }

    /// 保留中の tap が Flutter 相当の press timeout に達したら semantic pointer-down を
    /// Core へ送る。新しい visual work を発生させた場合だけ `true`。
    pub(crate) fn advance_press(&mut self, tree: &mut ElementTree, now_ms: f64) -> bool {
        let Some((_, _, started_ms)) = self.pending_pointer_down else {
            return false;
        };
        if now_ms - started_ms < scroll::PRESS_TIMEOUT_MS {
            return false;
        }
        let (x, y, _) = self.pending_pointer_down.take().expect("pending press checked above");
        tree.on_pointer_down_with_kind(x, y, 0, PointerKind::Touch);
        true
    }

    /// ヒットした要素の最近接 scroll-view 祖先にドラッグ→スクロールジェスチャを
    /// ロックする。競合する押下は Flutter と同様に timeout／tap 確定まで保留し、
    /// slop を越えなければリリース時に通常のクリックとして解決する。
    fn on_down(&mut self, tree: &mut ElementTree, x: f32, y: f32, now_ms: f64) {
        self.scroll_gesture = None;
        self.pending_pointer_down = None;
        self.drag_raw = None;
        self.scroll_samples.clear();
        if let Some(sv) = tree
            .hit_test(x, y)
            .and_then(|hit| tree.nearest_scroll_view(hit))
        {
            tree.prepare_deferred_pointer_down(PointerKind::Touch);
            self.scroll_gesture = Some(ScrollGesture::new(sv, (x, y)));
            self.pending_pointer_down = Some((x, y, now_ms));
        } else {
            tree.on_pointer_down_with_kind(x, y, 0, PointerKind::Touch);
        }
    }

    fn on_move(&mut self, tree: &mut ElementTree, x: f32, y: f32, now_ms: f64) {
        let Some(mut gesture) = self.scroll_gesture.take() else {
            let _ = tree.on_pointer_move_with_kind(x, y, PointerKind::Touch);
            return;
        };
        match gesture.on_move((x, y), self.tuning.slop_px) {
            // まだ slop デッドゾーン内 — 押下を生かしたままにする。
            MoveOutcome::Pending => {}
            // slop 超過: 押下を解除してスクロールへ移行し、リリースでクリックを
            // 発火させない。
            MoveOutcome::StartScroll => {
                if self.pending_pointer_down.take().is_none() {
                    tree.on_pointer_cancel();
                }
                self.scroll_samples.push((x, y, now_ms));
            }
            // ロックした scroll-view を指でドラッグする（範囲内は 1:1、端を越えると
            // ラバーバンドで抵抗）。リリース時のフリック推定用にサンプルを記録する。
            MoveOutcome::Scroll { dx, dy } => {
                self.apply_drag_delta(tree, gesture.scroll_view, dx, dy);
                self.scroll_samples.push((x, y, now_ms));
            }
        }
        self.scroll_gesture = Some(gesture);
    }

    fn on_up(&mut self, tree: &mut ElementTree, x: f32, y: f32) {
        match self.scroll_gesture.take() {
            Some(gesture) if !gesture.is_tap() => {
                self.launch_scroll_motion(tree, gesture.scroll_view)
            }
            _ => {
                if let Some((down_x, down_y, _)) = self.pending_pointer_down.take() {
                    tree.on_pointer_down_with_kind(down_x, down_y, 0, PointerKind::Touch);
                }
                tree.on_pointer_up_with_kind(x, y, PointerKind::Touch);
            }
        }
    }

    fn on_cancel(&mut self, tree: &mut ElementTree) {
        self.scroll_gesture = None;
        let pending = self.pending_pointer_down.take().is_some();
        self.drag_raw = None;
        self.scroll_samples.clear();
        if !pending {
            tree.on_pointer_cancel();
        }
    }

    /// `sv` の軸別スクロール境界 `(max_x, max_y, dim_x, dim_y)`。`max` はスクロール
    /// 可能範囲、`dim` はラバーバンドのオーバースクロールが漸近するビューポート寸法。
    fn scroll_bounds(tree: &ElementTree, sv: ElementId) -> (f32, f32, f32, f32) {
        let (max_x, max_y) = tree.element_scroll_max_offset(sv);
        let (_, _, view_w, view_h) = tree.element_layout_rect(sv).unwrap_or((0.0, 0.0, 0.0, 0.0));
        (max_x, max_y, view_w, view_h)
    }

    /// ロックした scroll-view のオフセットをクランプせず設定し（SCR-02）、実際に
    /// 動いたときは `Event::Scroll` を発火する（`on_wheel` 経由）。
    fn commit_scroll_offset(tree: &mut ElementTree, sv: ElementId, nx: f32, ny: f32) {
        let (ox, oy) = tree.element_get_scroll_offset(sv);
        let (dx, dy) = (nx - ox, ny - oy);
        if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
            tree.element_set_scroll_offset(sv, nx, ny);
            tree.on_wheel(sv, dx, dy);
        }
    }

    /// 指のドラッグ差分をラバーバンド経由でロックした scroll-view に適用する。指は
    /// 生のオフセットを 1:1 で動かし、表示オフセットは `rubber_band_offset(raw, …)`。
    fn apply_drag_delta(&mut self, tree: &mut ElementTree, sv: ElementId, dx: f32, dy: f32) {
        let (max_x, max_y, dim_x, dim_y) = Self::scroll_bounds(tree, sv);
        let (rx, ry) = match self.drag_raw {
            Some((s, raw)) if s == sv => raw,
            _ => tree.element_get_scroll_offset(sv),
        };
        let (rx, ry) = (rx + dx, ry + dy);
        self.drag_raw = Some((sv, (rx, ry)));
        // 実際にスクロールできる軸だけラバーバンドする（スクロール不可な軸は原点固定）。
        let nx = if max_x > 0.0 {
            scroll::rubber_band_offset(rx, max_x, dim_x, &self.tuning)
        } else {
            0.0
        };
        let ny = if max_y > 0.0 {
            scroll::rubber_band_offset(ry, max_y, dim_y, &self.tuning)
        } else {
            0.0
        };
        Self::commit_scroll_offset(tree, sv, nx, ny);
    }

    /// スクロールジェスチャのリリース時、記録した指サンプルから推定したフリック
    /// 速度でリリース運動を起動する。減衰・spring-back の毎フレーム積分は Core
    /// （`tree.render`）が所有する。
    fn launch_scroll_motion(&mut self, tree: &mut ElementTree, sv: ElementId) {
        let (vx, vy) = scroll::estimate_release_velocity(&self.scroll_samples, &self.tuning);
        self.scroll_samples.clear();
        self.drag_raw = None;
        tree.start_scroll_momentum(sv, vx, vy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{
        Color, Dimension, DocumentEventKind, ElementKind, Event, PseudoState, StyleProp,
    };

    /// 縦 500px のコンテンツを高さ 100px の ScrollView に入れた、縦スクロール可能な
    /// ツリー（`scroll_momentum_continuation.rs` と同型のフィクスチャ）。
    fn scrollable() -> (ElementTree, ElementId) {
        let mut tree = ElementTree::new();
        let scroll = tree.element_create(1, ElementKind::ScrollView);
        let content = tree.element_create(2, ElementKind::View);
        tree.set_root(scroll);
        tree.set_viewport(200.0, 100.0);
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

    fn scrollable_with_active_style() -> (ElementTree, ElementId) {
        let (mut tree, scroll) = scrollable();
        let content = tree.hit_test(100.0, 50.0).expect("fixture content must be hittable");
        tree.element_set_pseudo_style(
            content,
            PseudoState::Active,
            &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
        );
        tree.render(1.0);
        (tree, scroll)
    }

    #[test]
    fn fast_drag_never_activates_the_pending_tap_or_dirties_scroll_content() {
        let (mut tree, scroll) = scrollable_with_active_style();
        let mut state = TouchScrollState::new();

        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 50.0 }, 0.0);
        tree.render(0.0);
        assert_eq!(tree.active_element(), None, "scroll と競合中の tap はまだ active でない");
        assert!(
            tree.frame_layer_dirty().is_empty(),
            "保留中の press は scroll content cache を dirty にしてはならない",
        );

        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 30.0 }, 16.0);
        tree.render(16.0);
        assert_eq!(tree.active_element(), None, "fast drag は pending tap を棄却する");
        assert!(
            !tree.frame_layer_dirty().contains(&scroll),
            "slop 超過時にも active cancel の content raster を発生させてはならない",
        );
    }

    #[test]
    fn held_touch_activates_after_press_timeout_and_later_drag_cancels_it() {
        let (mut tree, scroll) = scrollable_with_active_style();
        let mut state = TouchScrollState::new();

        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 50.0 }, 0.0);
        assert!(!state.advance_press(&mut tree, 99.0));
        assert_eq!(tree.active_element(), None, "timeout 前は press を保留する");

        assert!(state.advance_press(&mut tree, 100.0));
        assert!(tree.active_element().is_some(), "100ms hold で active を表示する");
        tree.render(100.0);
        assert!(
            tree.frame_layer_dirty().contains(&scroll),
            "表示された active style は scroll content cache を更新する",
        );

        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 30.0 }, 116.0);
        assert_eq!(tree.active_element(), None, "active 後に drag と確定したら cancel する");
    }

    #[test]
    fn dragging_past_slop_over_content_scrolls_it_one_to_one() {
        let (mut tree, scroll) = scrollable();
        let mut state = TouchScrollState::new();

        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 50.0 }, 0.0);
        // 最初の move で slop（8px）を越える: この move 自体は遷移フレームで
        // デッドゾーンを消費し、オフセットは動かない（`MoveOutcome::StartScroll`）。
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 30.0 }, 16.0);
        // 続く move はスクロール中なので、指の差分だけ範囲内で 1:1 追従するはず。
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 10.0 }, 32.0);

        let (_, oy) = tree.element_get_scroll_offset(scroll);
        assert_eq!(oy, 20.0, "スクロール中のドラッグは範囲内で 1:1 追従するはず");
    }

    #[test]
    fn a_move_within_slop_still_resolves_as_a_tap_click_on_release() {
        let (mut tree, scroll) = scrollable();
        let listener = tree.register_listener(scroll, DocumentEventKind::Click);
        let mut state = TouchScrollState::new();

        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 50.0 }, 0.0);
        // slop（8px）未満の揺れ。まだ保留中のタップのまま。
        state.apply(&mut tree, PointerInput::Move { x: 103.0, y: 52.0 }, 16.0);
        state.apply(&mut tree, PointerInput::Up { x: 103.0, y: 52.0 }, 32.0);

        let clicks: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .filter(|d| matches!(d.event, Event::Click { .. }))
            .collect();
        assert_eq!(clicks.len(), 1, "slop 未満のドラッグは通常のタップ→クリックのはず");
        assert_eq!(clicks[0].listener_id, listener);

        let (_, oy) = tree.element_get_scroll_offset(scroll);
        assert_eq!(oy, 0.0, "タップはスクロールオフセットを動かしてはならない");
    }

    #[test]
    fn releasing_after_a_flick_keeps_scrolling_via_momentum() {
        let (mut tree, scroll) = scrollable();
        let mut state = TouchScrollState::new();

        // 速い上フリック: 3 サンプルで 30px/16ms 相当の速度を作る。
        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 90.0 }, 0.0);
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 60.0 }, 16.0);
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 30.0 }, 32.0);
        state.apply(&mut tree, PointerInput::Up { x: 100.0, y: 30.0 }, 48.0);

        assert!(
            tree.has_pending_visual_work(),
            "フリック解放直後は慣性の継続フレームを要求しなければならない",
        );
        let (_, oy_at_release) = tree.element_get_scroll_offset(scroll);

        let mut t = 64.0;
        while tree.has_pending_visual_work() {
            tree.render(t);
            t += 16.0;
            assert!(t < 5_000.0, "慣性は有限時間で静止しなければならない");
        }

        let (_, oy_after) = tree.element_get_scroll_offset(scroll);
        assert!(
            oy_after > oy_at_release,
            "慣性がリリース後もコンテンツを動かし続けたはず（{oy_at_release} -> {oy_after}）",
        );
    }

    #[test]
    fn grabbing_during_active_momentum_immediately_overrides_it() {
        // AC: 前のフリックの慣性がまだ続いている最中に新しい指で掴んだら、慣性の続きに
        // 打ち勝って新しいドラッグの指位置へ即座に一致しなければならない（慣性が自然に
        // 止まるまで新しい操作が無視される、という退行を防ぐ）。
        let (mut tree, scroll) = scrollable();
        let mut state = TouchScrollState::new();

        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 90.0 }, 0.0);
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 60.0 }, 16.0);
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 30.0 }, 32.0);
        state.apply(&mut tree, PointerInput::Up { x: 100.0, y: 30.0 }, 48.0);

        // 慣性を数フレーム進める（まだ止まっていない状態を作る）。
        let mut t = 64.0;
        for _ in 0..3 {
            tree.render(t);
            t += 16.0;
        }
        assert!(tree.has_pending_visual_work(), "この時点でまだ慣性が続いているはず");
        let (_, oy_mid_momentum) = tree.element_get_scroll_offset(scroll);

        // 慣性が止まる前に、新しい指で掴んで下方向にドラッグする
        // （down→10px→slop超過の遷移フレーム→実ドラッグ適用、の順は他のテストと同じ配線）。
        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 20.0 }, t);
        t += 16.0;
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 30.0 }, t);
        t += 16.0;
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 60.0 }, t);
        t += 16.0;
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 90.0 }, t);

        tree.render(t);
        let (_, oy_after) = tree.element_get_scroll_offset(scroll);

        // 実ドラッグ適用は最後の 2 move（30px+30px=60px）だけなので、期待値は mid - 60。
        assert!(
            (oy_after - (oy_mid_momentum - 60.0)).abs() < 1.0,
            "新しいドラッグが慣性の続きに打ち勝てていない: \
             mid={oy_mid_momentum}, expected≈{}, got={oy_after}",
            oy_mid_momentum - 60.0
        );
    }

    #[test]
    fn dragging_past_the_scroll_end_applies_rubber_band_resistance() {
        let (mut tree, scroll) = scrollable();
        let mut state = TouchScrollState::new();
        // このフィクスチャの縦スクロール可能範囲は 500 - 100 = 400px。押下はビューポート内
        // （0..100px）でなければヒットしない——スクロールジェスチャは pointerdown 時の
        // ヒットテストでロックする（ADR-0082）ので、以降の move 座標は画面外でもよい。
        state.apply(&mut tree, PointerInput::Down { x: 100.0, y: 50.0 }, 0.0);
        // 遷移フレーム（slop 消費、オフセット不変）。
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: 40.0 }, 16.0);
        // 端（400px）を 200px 越える生ドラッグ（400 + 200 = 600px 相当）。
        state.apply(&mut tree, PointerInput::Move { x: 100.0, y: -560.0 }, 32.0);

        let (_, oy) = tree.element_get_scroll_offset(scroll);
        assert!(
            oy > 400.0,
            "端を越えたら生ドラッグの分だけ max を越えて動くはず（got {oy}）"
        );
        assert!(
            oy < 600.0,
            "ラバーバンド抵抗で生の指移動量より遅れるはず（got {oy}）"
        );
    }
}
