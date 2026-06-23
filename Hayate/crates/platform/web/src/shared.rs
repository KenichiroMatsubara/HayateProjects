//! Canvas Mode（`canvas.rs`）と HTML Mode（`html.rs`）で共有するコード。
//! 分割の根拠は ADR-0077 を参照。

use hayate_core::{ElementId, ElementKind};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Document;

pub(crate) fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}

pub(crate) fn element_id_from_f64(raw: f64) -> ElementId {
    ElementId::from_u64(raw as u64)
}

pub(crate) fn element_id_to_f64(id: ElementId) -> f64 {
    id.to_u64() as f64
}

pub(crate) fn kind_from_u32(v: u32) -> Result<ElementKind, JsValue> {
    ElementKind::from_u32(v).ok_or_else(|| JsValue::from_str(&format!("unknown element kind {v}")))
}

// ── スタイルタグ定数（JS へ公開） ──────────────────────────────────

#[wasm_bindgen]
pub fn style_tag_z_index() -> u32 {
    crate::generated::TAG_Z_INDEX
}
#[wasm_bindgen]
pub fn style_tag_font_family() -> u32 {
    crate::generated::TAG_FONT_FAMILY
}

/// URL から生バイト列を取得する。
pub(crate) async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    use js_sys::{ArrayBuffer, Uint8Array};
    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await?
        .dyn_into()?;
    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "fetch failed: {} {}",
            resp.status(),
            resp.status_text()
        )));
    }
    let buf: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
    Ok(Uint8Array::new(&buf).to_vec())
}
