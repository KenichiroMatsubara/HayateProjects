//! capability 契約共通の typed エラー（ADR-0118）。
//!
//! capability メソッドは原則 `Result<T, CapabilityError>` を返す。Flutter
//! `platform_interface` が未実装で `UnimplementedError` を throw するのを、Rust では
//! `Err` への写像で表す（panic ではない — leaf は Kotlin/Swift への FFI 境界で、Rust
//! panic は FFI 越えで abort/UB）。scaffold stub は `Unimplemented` を返す。
//!
//! variant は最小 seed の 3 つ。`PermissionDenied` 等は最初の権限ゲート付き capability
//! を実機実装する時に足す（error variant にも「先置きしない」を適用・ADR-0118）。

use std::fmt;

/// capability 呼び出しの失敗。`Unimplemented`（scaffold 済み・未実装）/ `Unsupported`
/// （その platform に概念が無い）/ `Platform`（native 呼び出しの実行時失敗）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityError {
    /// scaffold されているが未実装（= Flutter `UnimplementedError`）。stub が返す。
    Unimplemented {
        capability: &'static str,
        platform: &'static str,
    },
    /// その platform に概念が無い（= Flutter の null / "not available"）。
    Unsupported {
        capability: &'static str,
        platform: &'static str,
    },
    /// native 呼び出しが実行時失敗（= Flutter `PlatformException`）。
    Platform { code: i32, message: String },
}

impl CapabilityError {
    /// `Unimplemented` の簡約コンストラクタ（stub から呼ぶ）。
    pub const fn unimplemented(capability: &'static str, platform: &'static str) -> Self {
        Self::Unimplemented {
            capability,
            platform,
        }
    }

    /// `Unsupported` の簡約コンストラクタ。
    pub const fn unsupported(capability: &'static str, platform: &'static str) -> Self {
        Self::Unsupported {
            capability,
            platform,
        }
    }
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unimplemented {
                capability,
                platform,
            } => write!(f, "capability `{capability}` is not yet implemented on {platform}"),
            Self::Unsupported {
                capability,
                platform,
            } => write!(f, "capability `{capability}` is not supported on {platform}"),
            Self::Platform { code, message } => {
                write!(f, "platform error {code}: {message}")
            }
        }
    }
}

impl std::error::Error for CapabilityError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unimplemented_constructor_carries_capability_and_platform() {
        let e = CapabilityError::unimplemented("clipboard", "android");
        assert_eq!(
            e,
            CapabilityError::Unimplemented {
                capability: "clipboard",
                platform: "android"
            }
        );
    }

    #[test]
    fn display_distinguishes_the_three_variants() {
        assert!(CapabilityError::unimplemented("clipboard", "ios")
            .to_string()
            .contains("not yet implemented"));
        assert!(CapabilityError::unsupported("haptics", "ios")
            .to_string()
            .contains("not supported"));
        assert!(CapabilityError::Platform {
            code: 7,
            message: "boom".into()
        }
        .to_string()
        .contains("platform error 7"));
    }
}
