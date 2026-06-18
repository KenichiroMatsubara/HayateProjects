use crate::element::style::{CursorValue, UserSelectValue};

/// Element-kind tables generated from `proto/spec/element_kinds.json` — the
/// single source for per-kind UA defaults (ADR-0105/ADR-0108). Brings
/// `ElementKind`, `CursorValue` and `UserSelectValue` into the generated
/// module's `super::` scope.
mod tables {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/element_kind_tables.rs"
    ));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    View,
    Text,
    Image,
    Button,
    TextInput,
    ScrollView,
}

impl ElementKind {
    /// UA default cursor for this kind when no explicit `cursor` is set
    /// (ADR-0105): `button` → pointer, `text-input` → text (I-beam), others →
    /// default. Sourced from `proto/spec/element_kinds.json` so Canvas and DOM
    /// share one table and neither renderer re-declares it.
    pub fn default_cursor(self) -> CursorValue {
        tables::default_cursor(self)
    }

    /// UA default `user-select` for this kind when no explicit `user-select` is
    /// set (ADR-0108): `view` / `text` / `scroll-view` / `text-input` are
    /// selectable (`Text`), `image` / `button` are not (`None`). Sourced from
    /// `proto/spec/element_kinds.json` so Canvas and DOM share one table and
    /// neither renderer re-declares the kind-default selectability.
    pub fn default_user_select(self) -> UserSelectValue {
        tables::default_user_select(self)
    }

    /// Whether this kind accepts text entry and so should surface the platform
    /// soft keyboard / IME when focused (#392). `true` for `text-input` only;
    /// plain `text` carries styles (Text-Local Carrier) but is not editable.
    /// Sourced from `proto/spec/element_kinds.json` so every adapter shares one
    /// table rather than re-deriving "is this a text field" per platform.
    pub fn accepts_text_input(self) -> bool {
        tables::accepts_text_input(self)
    }
}

impl ElementKind {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::View),
            1 => Some(Self::Text),
            2 => Some(Self::Image),
            3 => Some(Self::Button),
            4 => Some(Self::TextInput),
            5 => Some(Self::ScrollView),
            _ => None,
        }
    }

    pub fn is_text_like(self) -> bool {
        matches!(self, Self::Text | Self::Button | Self::TextInput)
    }
}
