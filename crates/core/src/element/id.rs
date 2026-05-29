#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ElementId(u64);

impl ElementId {
    pub fn from_u64(raw: u64) -> Self {
        Self(raw)
    }

    pub fn to_u64(self) -> u64 {
        self.0
    }
}
