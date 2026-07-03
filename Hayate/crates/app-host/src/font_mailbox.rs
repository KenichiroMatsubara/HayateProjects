//! フォント取得完了報告用の mailbox（ADR-0132 スライス2）。`FontFetcher`（capability
//! contract）とは別の理由で必要になる機構 — 単一スレッド WASM で `spawn_local` された
//! 非同期クロージャが `&mut ElementTree` を安全に横断 borrow できない（tick ループが
//! 排他的に tree を持つ間しか安全に書けない）再入問題を避けるため。`AppHost` が
//! 構築時に [`FontMailboxHandle`] を公開し、アダプタの `impl FontFetcher` はそれを
//! 保持して非同期クロージャ内から結果を push する。[`AppHost::tick`](crate::AppHost::tick)
//! は毎フレーム、layout より前にこの mailbox を drain する。

use std::sync::{Arc, Mutex};

/// アダプタの非同期フォント取得が完了した結果。
pub enum FontFetchResult {
    /// 取得成功。`family` の名前でバイト列を core へ登録する。
    Loaded { family: String, bytes: Vec<u8> },
    /// 取得失敗。`family` のリトライ予算を core へ報告する。
    Failed { family: String },
}

/// `AppHost` が所有する完了報告キュー。
pub struct FontMailbox {
    inner: Arc<Mutex<Vec<FontFetchResult>>>,
}

impl FontMailbox {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// アダプタの `impl FontFetcher` へ渡す clone 可能な書き込みハンドル。
    pub fn handle(&self) -> FontMailboxHandle {
        FontMailboxHandle {
            inner: self.inner.clone(),
        }
    }

    /// 溜まった結果を取り出す。`AppHost::tick` が layout より前に呼ぶ。`AppHost` を
    /// 介さずこの mailbox を直接所有するホスト（例: `hayate-adapter-web` の
    /// `HayateElementRenderer`）向けに `pub`。
    pub fn drain(&self) -> Vec<FontFetchResult> {
        std::mem::take(&mut *self.inner.lock().expect("font mailbox mutex poisoned"))
    }
}

impl Default for FontMailbox {
    fn default() -> Self {
        Self::new()
    }
}

/// [`FontMailbox`] への clone 可能な書き込みハンドル。非同期取得クロージャ内から
/// 結果を push するために使う。
#[derive(Clone)]
pub struct FontMailboxHandle {
    inner: Arc<Mutex<Vec<FontFetchResult>>>,
}

impl FontMailboxHandle {
    /// `family` の取得成功をキューする。
    pub fn report_loaded(&self, family: String, bytes: Vec<u8>) {
        self.inner
            .lock()
            .expect("font mailbox mutex poisoned")
            .push(FontFetchResult::Loaded { family, bytes });
    }

    /// `family` の取得失敗をキューする。
    pub fn report_failed(&self, family: String) {
        self.inner
            .lock()
            .expect("font mailbox mutex poisoned")
            .push(FontFetchResult::Failed { family });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_reports_are_visible_via_drain() {
        let mailbox = FontMailbox::new();
        let handle = mailbox.handle();
        handle.report_loaded("Test".to_string(), vec![1, 2, 3]);
        handle.report_failed("Other".to_string());

        let drained = mailbox.drain();
        assert_eq!(drained.len(), 2);
        assert!(matches!(
            &drained[0],
            FontFetchResult::Loaded { family, bytes } if family == "Test" && bytes == &[1, 2, 3]
        ));
        assert!(matches!(
            &drained[1],
            FontFetchResult::Failed { family } if family == "Other"
        ));
    }

    #[test]
    fn drain_empties_the_mailbox() {
        let mailbox = FontMailbox::new();
        mailbox.handle().report_failed("X".to_string());
        assert_eq!(mailbox.drain().len(), 1);
        assert!(mailbox.drain().is_empty());
    }
}
