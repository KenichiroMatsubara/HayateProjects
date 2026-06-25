//! secure storage capability 契約（ADR-0119）。モデル: `flutter_secure_storage`
//! （`read(key)` / `write(key, value)` / `delete(key)`）。実装は Keychain/Keystore（実装時）。

use crate::capability::CapabilityError;

/// 鍵付きの安全な文字列ストレージ。未登録キーの read は `Ok(None)`。
pub trait SecureStorage {
    fn read(&self, key: &str) -> Result<Option<String>, CapabilityError>;
    fn write(&mut self, key: &str, value: &str) -> Result<(), CapabilityError>;
    fn delete(&mut self, key: &str) -> Result<(), CapabilityError>;
}
