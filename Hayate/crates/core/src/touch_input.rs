//! プラットフォーム非依存の touch 変換 fold（ADR-0117 フェーズ1）。
//!
//! native のタッチ入力は、各 leaf が広い native タッチ enum（UIKit の `UITouch.phase`、
//! android-activity の `MotionAction`）を単一ポインタの [`TouchAction`]（Down/Move/Up/
//! Cancel）へ畳んだうえで、座標ベースの `hayate-core` ポインタ API（`on_pointer_down` /
//! `on_pointer_move` / `on_pointer_up`、ADR-0082 によりポインタ種別非依存）へ写す。
//!
//! その後半 — `TouchAction` + 座標 → [`PointerInput`] の fold — は platform-free で、
//! かつては `hayate-adapter-android`（80 行）と `hayate-adapter-ios`（86 行）の双方に同型の
//! まま複製されていた。本モジュールがその単一の正本を持ち、各 leaf には native enum →
//! [`TouchAction`] の写像だけを残す。
//!
//! gesture 認識（slop / tap-vs-scroll、ADR-0066）と scroll 物理（ADR-0113）は本 fold とは
//! 層が異なり、ここでは扱わない。座標は leaf が論理 points（iOS）/ サーフェスピクセル
//! （Android）のまま渡す既存方針を維持し、本 fold は座標を解釈せずそのまま透過する。
//!
//! 実機 SDK や DOM/wasm を要さず全ターゲットでコンパイル/テストできる純粋関数。

/// 単一ポインタのタッチアクション。native のタッチ enum（`UITouch.phase` /
/// `MotionAction`）を leaf が畳んだ platform-free な表現。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchAction {
    Down,
    Move,
    Up,
    Cancel,
}

/// タッチアクションが写る、座標付きの `hayate-core` ポインタ呼び出し。座標は leaf が
/// 渡した空間（iOS = 論理 points / Android = サーフェスピクセル）のまま。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerInput {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up { x: f32, y: f32 },
    /// システムジェスチャ奪取・割り込み等によるキャンセル。座標非依存で
    /// `ElementTree::on_pointer_cancel` へ写す。
    Cancel,
}

/// 単一の [`TouchAction`] + 座標を、対応する `hayate-core` ポインタ呼び出しへ畳む。
pub fn translate_touch(action: TouchAction, x: f32, y: f32) -> PointerInput {
    match action {
        TouchAction::Down => PointerInput::Down { x, y },
        TouchAction::Move => PointerInput::Move { x, y },
        TouchAction::Up => PointerInput::Up { x, y },
        // Cancel は座標非依存で押下を解除する（`ElementTree::on_pointer_cancel`）。
        // pointer up へは畳まない —— クリックはリリースで確定する（ADR-0082）ため、
        // up に写すとシステム奪取で消えた押下が誤ってクリックになってしまう。
        TouchAction::Cancel => PointerInput::Cancel,
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

    // Cancel（システムジェスチャによる奪取・通話割り込み・スクロール奪取など）は
    // 座標非依存で押下を解除する（`ElementTree::on_pointer_cancel`）。クリックは
    // リリースで確定する（ADR-0082）ので、pointer up へは畳まない —— up に写すと
    // 奪取で消えた押下が誤ってクリックになる。
    #[test]
    fn cancel_maps_to_pointer_cancel() {
        assert_eq!(
            translate_touch(TouchAction::Cancel, 7.0, 8.0),
            PointerInput::Cancel
        );
    }
}
