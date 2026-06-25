//! winit pointer 入力 → Core 座標 dispatch への純粋写像（ADR-0118 / issue #506）。
//!
//! winit の `CursorMoved` / `MouseInput`（press/release）を Core の座標ベース pointer
//! dispatch（`on_pointer_down` / `on_pointer_move` / `on_pointer_up`・内部 hit-test）へ写す。
//! desktop は mouse 入力のみ配線するので、Core には常に [`PointerKind::Mouse`] を渡す。
//!
//! winit イベント → 座標 dispatch 引数（座標・ボタン・kind）への変換は実窓・GPU 無しで
//! unit test できるよう純粋関数として切り出す。`MouseInput` は座標を運ばないため、
//! event loop は直近の `CursorMoved` 由来の論理座標を持ち回り、press/release に載せる。

use hayate_core::{ElementTree, PointerKind};
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton};

/// winit の物理ポインタ座標を Core のレイアウト座標（論理 px）へ変換する。これは Core の
/// ヒットテストと `layout_cache` が住む空間で、論理ビューポート（`set_viewport` 入力＝
/// `ViewportMetrics::viewport_size` ＝ 物理 / scale_factor）と同じ規約に揃える。
///
/// scale_factor で割らずに物理 px を渡すと、HiDPI（scale_factor ≠ 1）で Core が物理座標を
/// レイアウト座標と取り違え、ヒットテストを scale_factor 倍ぶん外す（クリックが意図の
/// scale_factor× の位置に着く）。dpr スケールは描画側で `content_scale` により別途適用される。
pub fn to_layout_coords(physical_x: f64, physical_y: f64, scale_factor: f64) -> (f32, f32) {
    let s = scale_factor as f32;
    (physical_x as f32 / s, physical_y as f32 / s)
}

/// Core への 1 件の座標 pointer dispatch。座標は論理（レイアウト）px。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerDispatch {
    /// `CursorMoved` 由来。hover／cursor を駆動する。
    Move { x: f32, y: f32 },
    /// primary（左）ボタンの press 由来。`:active` を点け、リリースでクリックを確定する。
    Down { x: f32, y: f32 },
    /// primary（左）ボタンの release 由来。
    Up { x: f32, y: f32 },
}

/// winit `CursorMoved` を Move dispatch へ写す。`position` は物理 px、`scale_factor` は
/// winit window の現在値。
pub fn cursor_moved_to_dispatch(
    position: PhysicalPosition<f64>,
    scale_factor: f64,
) -> PointerDispatch {
    let (x, y) = to_layout_coords(position.x, position.y, scale_factor);
    PointerDispatch::Move { x, y }
}

/// winit `MouseInput` を Down/Up dispatch へ写す。primary（左）ボタンのみ Core に渡し、
/// 右・中ボタン等は `None`（click/active を誤発火させない）。`MouseInput` は座標を運ばない
/// ので、`layout_pos`（直近の `CursorMoved` 由来の論理座標）を press/release に載せる。
pub fn mouse_input_to_dispatch(
    state: ElementState,
    button: MouseButton,
    layout_pos: (f32, f32),
) -> Option<PointerDispatch> {
    if button != MouseButton::Left {
        return None;
    }
    let (x, y) = layout_pos;
    Some(match state {
        ElementState::Pressed => PointerDispatch::Down { x, y },
        ElementState::Released => PointerDispatch::Up { x, y },
    })
}

/// dispatch を Core の座標 pointer API へ写す。desktop は mouse 入力のみ配線するので
/// 常に [`PointerKind::Mouse`] を渡す（ADR-0082/ADR-0104: Core は操作ごとに種別を保持）。
/// `Down` は修飾キー無し（`modifiers = 0`）。これが winit イベントループから tree への唯一の
/// pointer 入力経路。
pub fn apply_pointer_dispatch(tree: &mut ElementTree, dispatch: PointerDispatch) {
    match dispatch {
        PointerDispatch::Move { x, y } => {
            tree.on_pointer_move_with_kind(x, y, PointerKind::Mouse);
        }
        PointerDispatch::Down { x, y } => {
            tree.on_pointer_down_with_kind(x, y, 0, PointerKind::Mouse);
        }
        PointerDispatch::Up { x, y } => {
            tree.on_pointer_up_with_kind(x, y, PointerKind::Mouse);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{Color, Dimension, ElementId, ElementKind, PseudoState, StyleProp};

    /// 原点に置いた 100×40 の `kind` 単一要素を、`:hover`/`:active` のボックス見た目付きで
    /// レイアウト・レンダリング済みに返す（`(10,10)` がこの要素を必ずヒットする）。
    fn single_child(kind: ElementKind) -> (ElementTree, ElementId) {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(200.0, 200.0);
        let el = tree.element_create(2, kind);
        tree.element_set_style(
            el,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(40.0)),
            ],
        );
        tree.element_set_pseudo_style(
            el,
            PseudoState::Hover,
            &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
        );
        tree.element_set_pseudo_style(
            el,
            PseudoState::Active,
            &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
        );
        tree.element_append_child(root, el);
        tree.render(0.0);
        (tree, el)
    }

    #[test]
    fn dispatch_hover_then_press_drives_hover_active_with_mouse_kind() {
        // criteria #1/#3: hover で `:hover`、押下で `:active`、いずれも PointerKind=Mouse で
        // Core に届く。リリースで `:active` は解ける。
        let (mut tree, button) = single_child(ElementKind::Button);

        apply_pointer_dispatch(&mut tree, PointerDispatch::Move { x: 10.0, y: 10.0 });
        assert!(
            tree.interaction_snapshot().is_hovered(button),
            "hovering the button must enter :hover"
        );
        assert_eq!(
            tree.last_pointer_kind(),
            PointerKind::Mouse,
            "desktop pointer dispatch must carry PointerKind::Mouse"
        );

        apply_pointer_dispatch(&mut tree, PointerDispatch::Down { x: 10.0, y: 10.0 });
        assert!(
            tree.interaction_snapshot().is_active(button),
            "pressing the button must enter :active"
        );

        apply_pointer_dispatch(&mut tree, PointerDispatch::Up { x: 10.0, y: 10.0 });
        assert!(
            !tree.interaction_snapshot().is_active(button),
            "releasing must leave :active"
        );
    }

    #[test]
    fn dispatch_click_focuses_text_input_and_shows_focus_ring() {
        // criteria #2: text-input をクリックすると focus し、フォーカスリングが出る。
        let (mut tree, input) = single_child(ElementKind::TextInput);

        apply_pointer_dispatch(&mut tree, PointerDispatch::Down { x: 10.0, y: 10.0 });
        apply_pointer_dispatch(&mut tree, PointerDispatch::Up { x: 10.0, y: 10.0 });
        tree.render(16.0);

        assert_eq!(
            tree.focused_element(),
            Some(input),
            "clicking the text-input must focus it"
        );
        assert_eq!(
            tree.focus_visible_element(),
            Some(input),
            "a pointer-focused text-input must show the native focus ring"
        );
    }

    #[test]
    fn to_layout_coords_divides_physical_by_scale_factor() {
        // Core のヒットテストと layout_cache は論理（レイアウト）px に住む。winit の
        // CursorMoved は物理 px を運ぶので、scale_factor で割って論理座標へ落とす。
        // これを怠ると HiDPI（scale_factor ≠ 1）で全クリックが scale_factor 倍ずれ、
        // hit_test を外す。
        assert_eq!(to_layout_coords(200.0, 100.0, 2.0), (100.0, 50.0));
        // scale_factor = 1 では物理 = 論理（平行移動も拡大もしない）。
        assert_eq!(to_layout_coords(30.0, 40.0, 1.0), (30.0, 40.0));
    }

    #[test]
    fn cursor_moved_maps_to_move_in_layout_coords() {
        // winit CursorMoved（物理 px）は論理座標の Move dispatch になる。
        let d = cursor_moved_to_dispatch(PhysicalPosition::new(200.0, 100.0), 2.0);
        assert_eq!(d, PointerDispatch::Move { x: 100.0, y: 50.0 });
    }

    #[test]
    fn left_button_press_and_release_map_to_down_and_up_at_last_pos() {
        // MouseInput は座標を運ばないので、直近の CursorMoved 由来の論理座標を載せる。
        let pos = (40.0, 12.0);
        assert_eq!(
            mouse_input_to_dispatch(ElementState::Pressed, MouseButton::Left, pos),
            Some(PointerDispatch::Down { x: 40.0, y: 12.0 }),
        );
        assert_eq!(
            mouse_input_to_dispatch(ElementState::Released, MouseButton::Left, pos),
            Some(PointerDispatch::Up { x: 40.0, y: 12.0 }),
        );
    }

    #[test]
    fn non_primary_buttons_do_not_dispatch() {
        // primary（左）以外は Core に渡さない。右クリック等で click/active を誤発火させない。
        let pos = (40.0, 12.0);
        assert_eq!(
            mouse_input_to_dispatch(ElementState::Pressed, MouseButton::Right, pos),
            None,
        );
        assert_eq!(
            mouse_input_to_dispatch(ElementState::Pressed, MouseButton::Middle, pos),
            None,
        );
    }
}
