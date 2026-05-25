use slotmap::{Key, KeyData};

slotmap::new_key_type! {
    pub struct ElementId;
}

impl ElementId {
    pub fn to_u64(self) -> u64 {
        self.data().as_ffi()
    }

    pub fn from_u64(raw: u64) -> Self {
        Self::from(KeyData::from_ffi(raw))
    }
}
