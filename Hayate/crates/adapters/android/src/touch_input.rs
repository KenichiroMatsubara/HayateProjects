//! Android タッチ入力の変換（ADR-0087）。
//!
//! `android-activity` はイベントループ上で `MotionEvent` を届ける。本モジュールは
//! 単一ポインタのアクション + サーフェスピクセル座標を、座標ベースの `hayate-core`
//! ポインタ API（`on_pointer_down` / `on_pointer_move` / `on_pointer_up`、ADR-0082
//! によりポインタ種別非依存）へ写す。`android_activity`/`ndk` 型に依存させず、NDK
//! なしでホスト単体テストできる純粋関数にロジックを寄せ、汚いプラットフォーム
//! グルーを薄く保つ（`canvas_resize_metrics` や `ImeBridge` シームと同様）。

/// 単一ポインタのタッチアクション。`android_activity`/`ndk` 型に依存せず
/// Android の `MotionAction` を写す。
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchAction {
    Down,
    Move,
    Up,
    Cancel,
}

/// タッチアクションが写るサーフェスピクセル座標の `hayate-core` ポインタ呼び出し。
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerInput {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up { x: f32, y: f32 },
}

/// Android のタッチアクション1つ + サーフェスピクセル座標を、対応する
/// `hayate-core` ポインタ呼び出しへ変換する。
#[cfg(any(target_os = "android", test))]
pub fn translate_touch(action: TouchAction, x: f32, y: f32) -> PointerInput {
    match action {
        TouchAction::Down => PointerInput::Down { x, y },
        TouchAction::Move => PointerInput::Move { x, y },
        // Cancel はアクティブな押下を解除する（`on_pointer_cancel` はまだない）。
        TouchAction::Up | TouchAction::Cancel => PointerInput::Up { x, y },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_maps_to_pointer_down() {
        assert_eq!(
            translate_touch(TouchAction::Down, 10.0, 20.0),
            PointerInput::Down { x: 10.0, y: 20.0 }
        );
    }

    #[test]
    fn move_maps_to_pointer_move() {
        assert_eq!(
            translate_touch(TouchAction::Move, 33.0, 44.0),
            PointerInput::Move { x: 33.0, y: 44.0 }
        );
    }

    #[test]
    fn up_maps_to_pointer_up() {
        assert_eq!(
            translate_touch(TouchAction::Up, 5.0, 6.0),
            PointerInput::Up { x: 5.0, y: 6.0 }
        );
    }

    // Cancel（スクロール奪取やポインタキャプチャ喪失など）はキャンセル座標で
    // アクティブな押下を解除する。core にはまだ `on_pointer_cancel` がないため、
    // 最も近い既存挙動であるポインタ up にして `:active` の固着を防ぐ。
    #[test]
    fn cancel_maps_to_pointer_up() {
        assert_eq!(
            translate_touch(TouchAction::Cancel, 7.0, 8.0),
            PointerInput::Up { x: 7.0, y: 8.0 }
        );
    }
}
