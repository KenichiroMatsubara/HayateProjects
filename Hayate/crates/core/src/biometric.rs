//! biometric (local auth) capability 契約（ADR-0119）。モデル: `local_auth`
//! （`isDeviceSupported() -> bool` / `authenticate(reason) -> bool`）。失敗理由（cancel/lockout）
//! は今 `Platform` エラーに畳み、専用 outcome enum は実装時に足す（ADR-0119）。

use crate::capability::CapabilityError;

/// 生体認証（Face ID / 指紋）。`authenticate` の `bool` は認証成功か。
pub trait Biometric {
    fn is_available(&self) -> Result<bool, CapabilityError>;
    fn authenticate(&mut self, reason: &str) -> Result<bool, CapabilityError>;
}
