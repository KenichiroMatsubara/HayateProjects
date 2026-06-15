//! Clipboard write seam (ADR-0097, #268).
//!
//! Clipboard access is a Platform Adapter responsibility: the OS / browser
//! clipboard is reached through `web_sys`, JNI, AppKit, etc. The Element
//! Document Runtime owns *what* to copy (the selected text) but must not know
//! *how* it is written. This trait is that boundary — core depends only on the
//! trait, each Platform Adapter supplies the implementation.
//!
//! Writes are fire-and-forget: a browser `navigator.clipboard.writeText`
//! returns a promise, but core treats the call as a one-way request issued
//! during the user gesture (the Cmd/Ctrl+C keydown) that authorizes it.

/// A platform clipboard that core can write selected text into.
pub trait Clipboard {
    /// Write `text` to the system clipboard, replacing its contents.
    fn write_text(&self, text: &str);

    /// Read the system clipboard's current text, for a Paste action (ADR-0097
    /// decision 3: the Platform Adapter owns clipboard read *and* write). The
    /// default returns `None` so a write-only adapter still compiles; a browser
    /// adapter whose read is async resolves it out-of-band and feeds the result
    /// back through `element_paste`, leaving this `None`.
    fn read_text(&self) -> Option<String> {
        None
    }
}
