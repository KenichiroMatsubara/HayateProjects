//! Hermes(JSI/C++) ⇄ Rust の cxx ブリッジ（ADR-0112）。**device 未検証**。
//!
//! C++ 側（`cpp/hermes_app.cpp`）が Hermes ランタイムを作り、JS バンドルを eval し、
//! `globalThis.__hayateHost` として JSI HostObject を注入する。その HostObject の
//! 各メソッドは、ここで `extern "Rust"` 公開した [`JsHostBridge`] のメソッドへ
//! 降りて、ネイティブ Hayate（[`crate::js_host::JsHost`]）を駆動する。
//!
//! 逆方向（Rust→C++）では、app.rs が [`ffi::new_hermes_app`] でランタイムを作り、
//! 毎 vsync で `pump_frame` を呼ぶ。resize は native→tree 直結（app.rs が
//! `set_viewport` を直接駆動）で JS を経路から外したため、ここには無い（ADR-0080
//! を native へ延長, issue #475）。cxx のシグネチャ詳細（特に
//! `&[String]` の受け渡しや `Result` 変換）は device ビルドで微調整が要る可能性が
//! ある（この環境ではコンパイル検証できない）。
use std::cell::RefCell;
use std::rc::Rc;

use hayate_core::ElementTree;

use crate::js_host::{EventRow, JsHost};

#[cxx::bridge(namespace = "hayate")]
mod ffi {
    /// `poll_events` の配信原子（数値 or テキスト）。ADR-0053。
    struct FfiWireAtom {
        is_text: bool,
        number: f64,
        text: String,
    }

    /// 1 配信行 `[listener_id, kind, ...fields]`。
    struct FfiEventRow {
        atoms: Vec<FfiWireAtom>,
    }

    extern "Rust" {
        /// JSI HostObject が叩くネイティブ Hayate ハンドル。
        type JsHostBridge;

        fn apply_mutations(
            self: &JsHostBridge,
            ops: &[f64],
            styles: &[f32],
            texts: &CxxVector<CxxString>,
        ) -> Result<()>;
        fn render(self: &JsHostBridge, timestamp_ms: f64);
        fn register_listener(
            self: &JsHostBridge,
            element_id: f64,
            event_kind: u32,
        ) -> Result<f64>;
        fn element_get_text_content(self: &JsHostBridge, id: f64) -> String;
        fn element_subtree_ids(self: &JsHostBridge, id: f64) -> Vec<f64>;
        fn element_get_bounds(self: &JsHostBridge, id: f64) -> Vec<f32>;
        fn poll_events(self: &JsHostBridge) -> Vec<FfiEventRow>;
        fn has_pending_visual_work(self: &JsHostBridge) -> bool;
    }

    unsafe extern "C++" {
        include!("hayate-adapter-android/cpp/hermes_app.h");

        /// Hermes ランタイム + 注入済みホスト + ロード済みバンドルを保持する。
        type HermesApp;

        /// ランタイムを作り、`bundle`（JS ソース）を eval し、`host` を
        /// `globalThis.__hayateHost` として注入する。`globalThis.__tsubame` は
        /// バンドル側が公開する（main.android.tsx）。
        fn new_hermes_app(host: Box<JsHostBridge>, bundle: &str) -> UniquePtr<HermesApp>;

        /// `globalThis.__tsubame.pumpFrame(timestamp_ms)` を呼ぶ。続いて Hermes の
        /// マイクロタスクキューを排出する。
        fn pump_frame(self: Pin<&mut HermesApp>, timestamp_ms: f64);

        /// JS が `set_request_redraw` で登録したコールバックを呼ぶ（未登録なら no-op）。
        /// native の入力 wake（タッチ/IME）のたびに呼び、JS 側の frame ループの armed 状態
        /// （`HayateRenderer` の `pendingFrame`）を native の wake と揃える。
        fn request_redraw(self: Pin<&mut HermesApp>);

        /// JS が `request_pump` を呼んだか（＝JS 側の frame ループが armed になったか）を
        /// 読んで消費する。native のループは毎イテレーション呼び、true なら wake する。
        fn consume_wants_pump(self: Pin<&mut HermesApp>) -> bool;

        /// eval 済みバンドルが立てた `globalThis.__miharashiProtocolVersion` を読む（#533）。
        /// 有限数ならその値、未埋め込み / 非数値なら `-1.0`。`app_tsubame` がこれを `Option<u32>`
        /// に直し、`protocol_handshake::check_protocol_version` にかける。
        fn protocol_version(self: &HermesApp) -> f64;
    }
}

/// `JsHost` を cxx 越しに公開するためのラッパー。app.rs と tree を共有する。
pub struct JsHostBridge {
    host: JsHost,
}

impl JsHostBridge {
    fn apply_mutations(
        &self,
        ops: &[f64],
        styles: &[f32],
        texts: &cxx::CxxVector<cxx::CxxString>,
    ) -> Result<(), String> {
        let texts: Vec<String> = texts
            .iter()
            .map(|s| s.to_str().map(|s| s.to_owned()).unwrap_or_default())
            .collect();
        self.host.apply_mutations(ops, styles, &texts)
    }

    fn render(&self, timestamp_ms: f64) {
        self.host.render(timestamp_ms);
    }

    fn register_listener(&self, element_id: f64, event_kind: u32) -> Result<f64, String> {
        self.host.register_listener(element_id, event_kind)
    }

    fn element_get_text_content(&self, id: f64) -> String {
        self.host.element_get_text_content(id)
    }

    fn element_subtree_ids(&self, id: f64) -> Vec<f64> {
        self.host.element_subtree_ids(id)
    }

    fn element_get_bounds(&self, id: f64) -> Vec<f32> {
        self.host.element_get_bounds(id)
    }

    fn poll_events(&self) -> Vec<ffi::FfiEventRow> {
        self.host.poll_events().into_iter().map(ffi_row_from).collect()
    }

    fn has_pending_visual_work(&self) -> bool {
        self.host.has_pending_visual_work()
    }
}

fn ffi_row_from(row: EventRow) -> ffi::FfiEventRow {
    ffi::FfiEventRow {
        atoms: row
            .atoms
            .into_iter()
            .map(|a| ffi::FfiWireAtom {
                is_text: a.is_text,
                number: a.number,
                text: a.text,
            })
            .collect(),
    }
}

/// app.rs から: 共有 tree を握るブリッジを Box 化して C++ へ渡す。
pub(crate) fn make_bridge(tree: Rc<RefCell<ElementTree>>) -> Box<JsHostBridge> {
    Box::new(JsHostBridge {
        host: JsHost::new(tree),
    })
}

pub(crate) use ffi::{new_hermes_app, HermesApp};
