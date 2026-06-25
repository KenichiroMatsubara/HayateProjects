//! url launcher capability 契約（ADR-0119）。モデル: `url_launcher`
//! （`canLaunchUrl(uri) -> bool` / `launchUrl(uri) -> bool`）。

use crate::capability::CapabilityError;

/// 外部 URL/deep link を OS に開かせる。`launch` の `bool` はハンドルされたか。
pub trait UrlLauncher {
    fn can_launch(&self, url: &str) -> Result<bool, CapabilityError>;
    fn launch(&mut self, url: &str) -> Result<bool, CapabilityError>;
}
