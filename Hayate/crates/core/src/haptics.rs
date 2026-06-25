//! haptics capability 契約（ADR-0118）。モデル: Flutter `HapticFeedback`
//! （`lightImpact` / `mediumImpact` / `heavyImpact` / `selectionClick` / `vibrate`）。

use crate::capability::CapabilityError;

/// 触覚フィードバックの種類（Flutter `HapticFeedback` の写し）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HapticKind {
    LightImpact,
    MediumImpact,
    HeavyImpact,
    SelectionClick,
    Vibrate,
}

/// 単発の触覚フィードバックを発火する。
pub trait Haptics {
    fn feedback(&mut self, kind: HapticKind) -> Result<(), CapabilityError>;
}
