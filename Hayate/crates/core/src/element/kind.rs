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
