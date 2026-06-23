//! iOS leaf の touch glue（ADR-0114 / ADR-0117）。
//!
//! platform-free な touch 変換 fold（`TouchAction` + 座標 → 座標ベースの pointer dispatch）
//! は `hayate_core::touch_input` が単一の正本として所有する（ADR-0117 フェーズ1）。本モジュール
//! はその型を re-export し、iOS 固有の glue — FFI の `UITouch.phase`（`app.rs` の
//! `touch_phase_to_action`）→ [`TouchAction`] の写像 — だけを leaf に残す。
//!
//! 座標は `touch.location(in:view)` が返す論理 points をそのまま渡す。レイアウト/ヒット
//! テストも points 空間で走る（`surface_lifecycle` 参照）ため scale 乗算は不要。将来レイアウトを
//! 物理 px に移すなら、`surface_lifecycle` の scale 引数と同調してタッチ座標を再スケールする。

#[cfg_attr(not(target_os = "ios"), allow(unused_imports))]
pub use hayate_core::{translate_touch, PointerInput, TouchAction};
