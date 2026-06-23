//! Android leaf の touch glue（ADR-0087 / ADR-0117）。
//!
//! platform-free な touch 変換 fold（`TouchAction` + 座標 → 座標ベースの pointer dispatch）
//! は `hayate_core::touch_input` が単一の正本として所有する（ADR-0117 フェーズ1）。本モジュール
//! はその型を re-export し、Android 固有の glue — `android-activity` の `MotionAction`
//! （`app.rs` の `motion_action_to_touch`）→ [`TouchAction`] の写像 — だけを leaf に残す。
//!
//! 座標は content scale 1.0 のサーフェスピクセルのまま渡す既存方針を維持する
//! （`surface_lifecycle` 参照）。DPI 対応を入れる際は、`translate_touch` に渡すタッチ座標を
//! ビューポートと同じ scale で再スケールしてヒットテストと描画を揃える。

#[cfg_attr(not(target_os = "android"), allow(unused_imports))]
pub use hayate_core::{translate_touch, PointerInput, TouchAction};
