//! 要素種別の UA デフォルト + 明示 `user-select` → ブラウザ `user-select` への
//! マッピング（ADR-0108、ADR-0097 を置換）。HTML Mode は `user-select` で
//! 境界づけられたブラウザネイティブ選択を使う。Tsubame DOM Renderer
//! （`resolveUserSelect`）とのパリティ契約の Rust 側で、単一ソース
//! `proto/spec/fixtures/user_select_parity.json` が両側を固定する（ADR-0070）。

use hayate_core::{ElementKind, UserSelectValue};

/// 要素の CSS `user-select` 値を解決する。
///
/// 解決順序（ADR-0108）: 明示 `user-select` → 要素種別の UA デフォルト →
/// （none/選択不可）。選択可能性は CSS と同様にオプトアウト。
///
/// - text-input は常に編集選択を持つため、明示値や種別デフォルトに関わらず `text`。
/// - それ以外の実効値は、明示 `user-select` があればそれ、無ければ種別デフォルト
///   （`view` / `text` / `scroll-view` = `text`、`image` / `button` = `none`）。
/// - `text` は CSS `text`、`none` は CSS `none` へ。後者はサブツリーを除外する。
/// - `contains` は選択可能だが包含境界を確立するため CSS `contain` へ写す
///   （ADR-0108）。`user-select: contain` をサポートするブラウザはネイティブ選択を
///   要素内に閉じ込め、非サポートのブラウザはそれを無視するが、core 側の境界
///   クランプが同じセマンティクスを供給する（セマンティクスのみのパリティ）。
pub fn resolve_user_select(kind: ElementKind, explicit: Option<UserSelectValue>) -> &'static str {
    if matches!(kind, ElementKind::TextInput) {
        return "text";
    }
    let effective = explicit.unwrap_or_else(|| kind.default_user_select());
    match effective {
        UserSelectValue::Text => "text",
        UserSelectValue::Contains => "contain",
        UserSelectValue::None => "none",
    }
}
