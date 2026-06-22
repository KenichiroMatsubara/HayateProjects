//! iOS タッチ入力の変換（ADR-0113）。
//!
//! UIKit は `touchesBegan/Moved/Ended/Cancelled` で `UITouch` を届ける。本モジュールは
//! 単一ポインタの `UITouch.phase` + ビュー座標（points）を、座標ベースの `hayate-core`
//! ポインタ API（`on_pointer_down` / `on_pointer_move` / `on_pointer_up`、ADR-0082 により
//! ポインタ種別非依存）へ写す。`objc2`/UIKit 型に依存させず、Mac/SDK なしでホスト単体
//! テストできる純粋関数にロジックを寄せ、汚いプラットフォームグルーを薄く保つ
//! （`surface_lifecycle` / `ime_input` シームと同様）。`hayate-adapter-android` の
//! `touch_input` と同型で、`UITouch.phase` は Android の `MotionAction` と同じく
//! `TouchAction` に畳まれる。
//!
//! 座標は `touch.location(in:view)` が返す論理 points をそのまま渡す。レイアウト/ヒット
//! テストも points 空間で走る（`surface_lifecycle` 参照）ため scale 乗算は不要。将来
//! レイアウトを物理 px に移すなら、Android のコメント同様この seam に scale 引数を足して
//! ヒットテストと描画を揃える。

/// 単一ポインタのタッチアクション。`objc2`/UIKit 型に依存せず `UITouch.phase` を写す。
#[cfg(any(target_os = "ios", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchAction {
    Down,
    Move,
    Up,
    Cancel,
}

/// タッチアクションが写るビュー座標（points）の `hayate-core` ポインタ呼び出し。
#[cfg(any(target_os = "ios", test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerInput {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up { x: f32, y: f32 },
}

/// iOS のタッチアクション 1 つ + ビュー座標（points）を、対応する `hayate-core`
/// ポインタ呼び出しへ変換する。
#[cfg(any(target_os = "ios", test))]
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

    // Cancel（システムジェスチャによる奪取や通話割り込みなど）はキャンセル座標で
    // アクティブな押下を解除する。core にはまだ `on_pointer_cancel` がないため、最も
    // 近い既存挙動であるポインタ up にして `:active` の固着を防ぐ。
    #[test]
    fn cancel_maps_to_pointer_up() {
        assert_eq!(
            translate_touch(TouchAction::Cancel, 7.0, 8.0),
            PointerInput::Up { x: 7.0, y: 8.0 }
        );
    }
}
