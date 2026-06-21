//! クリップボード書き込みの境界（ADR-0097）。
//!
//! クリップボードアクセスは Platform Adapter の責務。OS / ブラウザの
//! クリップボードへは `web_sys`・JNI・AppKit 等を通じて到達する。Element
//! Document Runtime は「何を」コピーするか（選択テキスト）を持つが、
//! 「どう」書くかは知らない。この trait がその境界で、core は trait のみに
//! 依存し、実装は各 Platform Adapter が供給する。
//!
//! 書き込みは fire-and-forget。ブラウザの `navigator.clipboard.writeText` は
//! promise を返すが、core は許可を与えるユーザー操作（Cmd/Ctrl+C の keydown）
//! 中に発行する一方向リクエストとして扱う。

/// core が選択テキストを書き込めるプラットフォームのクリップボード。
pub trait Clipboard {
    /// `text` をシステムクリップボードへ書き込み、既存内容を置き換える。
    fn write_text(&self, text: &str);

    /// Paste 用にシステムクリップボードの現在のテキストを読む（ADR-0097:
    /// クリップボードの読み書きはどちらも Platform Adapter が持つ）。デフォルト
    /// は `None` を返すので書き込み専用アダプタもコンパイルできる。読み取りが
    /// 非同期のブラウザアダプタは別経路で解決し、結果を `element_paste` 経由で
    /// 返すため、ここは `None` のままにする。
    fn read_text(&self) -> Option<String> {
        None
    }
}
