//! 埋め込み Hermes（ADR-0112）が呼ぶ `RawHayate` のネイティブ実装（純 Rust）。
//!
//! C++/JSI ホスト（[`crate::hermes_bridge`]）は flat-C ABI 越しにここへ降りる。
//! 本モジュールは wasm に一切依存せず、Web の `HayateElementRenderer`
//! （`crates/adapters/web/src/canvas.rs`）が `self.tree` に対して行う操作を、
//! `Rc<RefCell<ElementTree>>` を介してネイティブ向けに写したもの。これにより
//! Tsubame Canvas Renderer がフレームごとに呼ぶ最小メソッド集合
//! （apply_mutations / render / poll_events / register_listener / on_resize /
//! element_get_text_content / element_subtree_ids / element_get_bounds）を満たす。
//!
//! 入力（タッチ/IME）は Android では native→tree 直結のまま（app.rs）で、JS を
//! 経由しないため `on_pointer_*` はここに含めない（ADR-0112）。
use std::cell::RefCell;
use std::rc::Rc;

use hayate_core::{DocumentEventKind, ElementId, ElementTree};

use crate::generated::{encode_event_wire, EventWireValue};
use crate::js_apply;

/// `poll_events` の配信行 1 要素（数値またはテキスト）。ADR-0053 の
/// `[listener_id, kind, ...fields]` をプラットフォーム非依存に表したもの。
pub(crate) struct WireAtom {
    pub is_text: bool,
    pub number: f64,
    pub text: String,
}

impl WireAtom {
    fn number(n: f64) -> Self {
        Self { is_text: false, number: n, text: String::new() }
    }
    fn text(s: String) -> Self {
        Self { is_text: true, number: 0.0, text: s }
    }
}

/// 1 配信 = `[listener_id, kind, ...fields]`。
pub(crate) struct EventRow {
    pub atoms: Vec<WireAtom>,
}

/// Tsubame Canvas Renderer が駆動するネイティブ Hayate ホスト。`tree` は
/// app.rs の vsync ループと共有する（単一スレッド, ADR-0003）。
pub(crate) struct JsHost {
    tree: Rc<RefCell<ElementTree>>,
}

impl JsHost {
    pub(crate) fn new(tree: Rc<RefCell<ElementTree>>) -> Self {
        Self { tree }
    }

    /// バッチ適用（ADR-0052）。共有の中立 dispatch を通す。
    pub(crate) fn apply_mutations(
        &self,
        ops: &[f64],
        styles: &[f32],
        texts: &[String],
    ) -> Result<(), String> {
        js_apply::apply_mutations(&mut self.tree.borrow_mut(), ops, styles, texts)
    }

    /// レイアウト + 保持シーンの lower（ADR-0086）。戻り値の `&SceneGraph` は
    /// `RefMut` に紐づくためここでは保持しない。app.rs が present 時に再取得する。
    pub(crate) fn render(&self, timestamp_ms: f64) {
        let _ = self.tree.borrow_mut().render(timestamp_ms);
    }

    /// ビューポート更新。Android の DPR は app.rs が物理ピクセルで吸収するため
    /// ここでは論理サイズを viewport に渡す。
    pub(crate) fn on_resize(&self, width: f32, height: f32, _scale: f32) {
        self.tree.borrow_mut().set_viewport(width, height);
    }

    /// リスナ登録（ADR-0053）。未知の event kind は Err。
    pub(crate) fn register_listener(
        &self,
        element_id: f64,
        event_kind: u32,
    ) -> Result<f64, String> {
        let kind = DocumentEventKind::from_u32(event_kind)
            .ok_or_else(|| format!("unknown event kind {event_kind}"))?;
        let id = self
            .tree
            .borrow_mut()
            .register_listener(ElementId::from_u64(element_id as u64), kind);
        Ok(id.to_u64() as f64)
    }

    /// 編集可能テキスト内容（input 配信時の値読み戻し）。
    pub(crate) fn element_get_text_content(&self, id: f64) -> String {
        self.tree
            .borrow()
            .element_get_text_content(ElementId::from_u64(id as u64))
    }

    /// `id` とその子孫の要素 id（remove 前の問い合わせ用）。
    pub(crate) fn element_subtree_ids(&self, id: f64) -> Vec<f64> {
        self.tree
            .borrow()
            .subtree_element_ids(ElementId::from_u64(id as u64))
            .into_iter()
            .map(|e| e.to_u64() as f64)
            .collect()
    }

    /// 直近レイアウトの絶対境界 `[x, y, width, height]`。未知/未レイアウトはゼロ。
    pub(crate) fn element_get_bounds(&self, id: f64) -> Vec<f32> {
        let (x, y, w, h) = self
            .tree
            .borrow()
            .element_layout_rect(ElementId::from_u64(id as u64))
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        vec![x, y, w, h]
    }

    /// 配信のポーリング（ADR-0053）。各行は `[listener_id, kind, ...fields]`。
    /// `FetchFont` 等の内部イベントはここで吸い出して握りつぶす（Android の
    /// フォントはネイティブ調達: 後続スライスで native 登録へ橋渡しする）。
    pub(crate) fn poll_events(&self) -> Vec<EventRow> {
        let mut tree = self.tree.borrow_mut();
        // 内部イベントキューを排出（web の poll_events と対称。FetchFont は将来
        // ネイティブフォント登録へ回す）。
        for event in tree.poll_events() {
            let _ = event; // TODO(ADR-0043): FetchFont をネイティブ調達へ。
        }
        tree.poll_deliveries()
            .into_iter()
            .map(|delivery| {
                let mut atoms = vec![WireAtom::number(delivery.listener_id.to_u64() as f64)];
                for v in encode_event_wire(&delivery.event) {
                    atoms.push(wire_atom_from(v));
                }
                EventRow { atoms }
            })
            .collect()
    }
}

fn wire_atom_from(v: EventWireValue) -> WireAtom {
    match v {
        EventWireValue::Number(n) => WireAtom::number(n),
        EventWireValue::Text(s) => WireAtom::text(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> JsHost {
        JsHost::new(Rc::new(RefCell::new(ElementTree::new())))
    }

    #[test]
    fn empty_apply_and_render_ok() {
        let h = host();
        assert!(h.apply_mutations(&[], &[], &[]).is_ok());
        h.on_resize(360.0, 640.0, 2.0);
        h.render(0.0);
        // 何も登録していなければ配信は空。
        assert!(h.poll_events().is_empty());
    }

    #[test]
    fn unknown_event_kind_errors() {
        let h = host();
        assert!(h.register_listener(1.0, 9999).is_err());
    }
}
