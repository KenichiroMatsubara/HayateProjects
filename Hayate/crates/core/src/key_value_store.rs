//! key-value storage capability 契約（ADR-0118）。モデル: `shared_preferences`
//! （typed getter 群）。scaffold は string 正準のみ — bool/int 等の typed メソッドは
//! 「先置きしない」で実装時に足す（ADR-0118）。

use crate::capability::CapabilityError;

/// 非機密な永続キー値ストア（string 正準）。未登録キーの get は `Ok(None)`。
pub trait KeyValueStore {
    fn get_string(&self, key: &str) -> Result<Option<String>, CapabilityError>;
    fn set_string(&mut self, key: &str, value: &str) -> Result<(), CapabilityError>;
    fn remove(&mut self, key: &str) -> Result<(), CapabilityError>;
    fn contains_key(&self, key: &str) -> Result<bool, CapabilityError>;
}
