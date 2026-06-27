//! iOS leaf の IME glue（ADR-0114 / ADR-0117）。
//!
//! IME の両入力モデル — 増分 command モデル（`ImeCommand` / `ImeBuffer` / `apply_command`）と
//! 絶対状態モデル（`translate_text_input`）、および共通出力（`ImeAction` / `apply_ime_action`、
//! コアの「確定 text_content + 末尾 preedit」モデル ADR-0069）— はすべて
//! `hayate_core::element` が単一の正本として所有する（ADR-0117 フェーズ1）。本モジュールは
//! iOS が使う増分側の型を re-export し、iOS 固有の glue — UITextInput コールバック
//! （`insertText:` / `deleteBackward` / `setMarkedText:` / `unmarkText`、`app.rs` の FFI）→
//! [`ImeCommand`] の写像 — だけを leaf に残す。
//!
//! Android leaf は同じ出力へ絶対状態 diff（GameTextInput buffer → `TextInputState`）で合流する。
//! どちらの leaf も編集セマンティクスを持たず、native callback / native buffer → コア入力型の
//! 薄い写像に徹する。ソフトキーボード表示の plumbing（`ImeBridge`）は `ime_bridge.rs` の leaf。
//!
//! キャレット連携（issue #563）: iOS の薄い glue（`app.rs` の `apply_command` →
//! `apply_ime_action`）は desktop と同じコア経路をそのまま使うので、確定/preedit を
//! **キャレット位置**へ入れる修正はコア側（`ime_command::apply_command` が末尾連結＋全文
//! `SetText` ではなくキャレット位置へ確定する `ImeAction::CommitText` を出す）で完結し、
//! iOS でも同時に解消される。回帰は core 側のテスト
//! `ime_command::tests::committing_at_mid_caret_lands_at_the_caret_not_the_tail` が
//! 共有経路（`apply_command`/`apply_ime_action`）越しに固定する。

#[cfg_attr(not(target_os = "ios"), allow(unused_imports))]
pub use hayate_core::element::ime_command::{apply_command, ImeBuffer, ImeCommand};
#[cfg_attr(not(target_os = "ios"), allow(unused_imports))]
pub use hayate_core::element::ime_reconcile::{apply_ime_action, ImeAction};
