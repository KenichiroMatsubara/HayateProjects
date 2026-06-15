//! Selection Region boundary → browser `user-select` mapping (ADR-0097
//! decision 5). HTML Mode uses the browser's native selection, bounded by
//! `user-select`. This is the Rust half of the parity contract with the
//! Tsubame DOM Renderer (`resolveUserSelect`); the single source
//! `proto/spec/fixtures/user_select_parity.json` pins both sides (ADR-0070).

use hayate_core::ElementKind;

/// Resolve the `user-select` value for an element.
///
/// - text-input is always selectable (editing requires it), regardless of any
///   Selection Region boundary.
/// - Otherwise an element is selectable only inside a `selectable` subtree
///   (`Some(true)`); the default (`None`) and an explicit `Some(false)` map to
///   `none`.
pub fn resolve_user_select(kind: ElementKind, selectable: Option<bool>) -> &'static str {
    if matches!(kind, ElementKind::TextInput) {
        return "text";
    }
    if selectable == Some(true) {
        "text"
    } else {
        "none"
    }
}
