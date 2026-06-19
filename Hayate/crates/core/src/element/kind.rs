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

    /// UA default *layout* for this kind, used as the base `taffy::Style` an
    /// element is created with so explicit props applied later (`element_set_style`)
    /// layer on top — the same resolution order as `default_cursor`: explicit >
    /// element-kind default > Taffy default (ADR-0109).
    ///
    /// `button` mirrors the browser `<button>`: content centered on the cross
    /// axis (`align-items: center`, vertical) and left-aligned on the main axis
    /// (`justify-content: flex-start`, horizontal). The horizontal default stays
    /// flex-start on purpose — centering it would regress left-aligned button
    /// labels (e.g. todo rows) and diverge from the DOM's `text-align: inherit`;
    /// a button that wants horizontal centering sets `justify-content: center`
    /// explicitly (ADR-0109 §1). Every other kind keeps the plain Taffy default.
    ///
    /// Unlike `default_cursor`, this is not sourced from `element_kinds.json`:
    /// kind layout defaults are a Taffy-`Style` concern with no TS/DOM consumer
    /// (the DOM gets `<button>` centering from the browser UA for free), so there
    /// is nothing to co-generate — hence an `enum`-local default, not a spec table
    /// (ADR-0109 §3).
    pub fn base_layout_style(self) -> taffy::Style {
        match self {
            Self::Button => taffy::Style {
                align_items: Some(taffy::AlignItems::Center),
                justify_content: Some(taffy::JustifyContent::FlexStart),
                ..taffy::Style::default()
            },
            // UA default for the scroll-view kind: it is a CSS scroll container,
            // the Canvas-side counterpart of the DOM renderer giving scroll-view
            // an `overflow: auto` tag default. A scroll container's flex
            // `min-{width,height}: auto` automatic minimum size resolves to 0
            // instead of its content size, so a scroll-view sized `height: 100%`
            // (or `flex-grow: 1`) shrinks to the space its siblings leave rather
            // than overflowing the parent by their extent — which would leave
            // that fixed band of content unreachable, since the inflated box
            // height is also the scroll viewport (`element_scroll_max_offset`).
            // The explicit `overflow` prop on any other kind takes the same
            // layout path via `apply_overflow_to_style`; this is just the kind
            // default (no `overflow` prop is set on scroll-views). `Scroll` with
            // the default `scrollbar_width: 0` reserves no space and lays out
            // like `Hidden`; the clip/scroll machinery lives in scene_build /
            // canvas.rs.
            Self::ScrollView => taffy::Style {
                overflow: taffy::Point {
                    x: taffy::Overflow::Scroll,
                    y: taffy::Overflow::Scroll,
                },
                ..taffy::Style::default()
            },
            _ => taffy::Style::default(),
        }
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
