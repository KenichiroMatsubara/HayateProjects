//! share capability 契約（ADR-0118）。モデル: `share_plus`（`Share.share(text, subject)`）。
//! ファイル共有（`shareXFiles`）は file picker のファイルモデルが固まってから（ADR-0118）。

use crate::capability::CapabilityError;

/// OS の共有シート（Android `ACTION_SEND` / iOS `UIActivityViewController`）にテキストを渡す。
pub trait Share {
    fn share_text(&mut self, text: &str, subject: Option<&str>) -> Result<(), CapabilityError>;
}
