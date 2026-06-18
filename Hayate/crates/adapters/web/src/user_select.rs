//! Element-kind UA default + explicit `user-select` → browser `user-select`
//! mapping (ADR-0108, supersedes ADR-0097 decision 2; refines decision 5). HTML
//! Mode uses the browser's native selection, bounded by `user-select`. This is
//! the Rust half of the parity contract with the Tsubame DOM Renderer
//! (`resolveUserSelect`); the single source
//! `proto/spec/fixtures/user_select_parity.json` pins both sides (ADR-0070).

use hayate_core::{ElementKind, UserSelectValue};

/// Resolve the CSS `user-select` value for an element.
///
/// Resolution order (ADR-0108): explicit `user-select` → element-kind UA
/// default → (none/unselectable). Selectability is opt-out, mirroring CSS:
///
/// - text-input always owns its editing selection, so it is `text` regardless
///   of any explicit value or kind default.
/// - Otherwise the effective value is the explicit `user-select` if present,
///   else the kind default (`view` / `text` / `scroll-view` = `text`,
///   `image` / `button` = `none`).
/// - `text` and `contains` are selectable and map to CSS `text` (`contains`
///   only adds a containment boundary, resolved core-side); `none` maps to CSS
///   `none` and excludes the subtree.
pub fn resolve_user_select(kind: ElementKind, explicit: Option<UserSelectValue>) -> &'static str {
    if matches!(kind, ElementKind::TextInput) {
        return "text";
    }
    let effective = explicit.unwrap_or_else(|| kind.default_user_select());
    match effective {
        UserSelectValue::Text | UserSelectValue::Contains => "text",
        UserSelectValue::None => "none",
    }
}
