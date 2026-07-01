//! Canvas が自前で配線する IME / EditContext（ADR-0069 / ADR-0080）。
//!
//! `pointer_input` / `resize_observer` と同型に、アダプタが `EditContext` を自己配線する。
//! `attach_edit_context`（wasm32 のみ）が `compositionstart` / `textupdate` /
//! `textformatupdate` / `compositionend` / `keydown` を購読し、生のイベントを順序付き
//! `pending_edit` バッファへ積む。`render()` 冒頭でドレインして core の編集シーム
//! （`on_composition_*` / `on_text_input` / `on_key_down`）を駆動し、`render()` 末尾で
//! core が決めた `ImePresentation`（`drive_ime`）を `EditContext` の着脱・候補窓 rect へ反映する。
//! JS ホスト（Tsubame）は IME 経路から外れる。
//!
//! 下線フォーマット変換（UTF-16→UTF-8）と候補窓 rect 変換は純粋関数で、全ターゲットで
//! 単体テストする。`EditContext` のラップと配線だけが wasm32 限定。

/// EditContext `textformatupdate.getTextFormats()` の変換フォーマット範囲1件。
/// オフセットは EditContext テキスト上の UTF-16 コードユニット位置。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClauseFormat {
    pub range_start: u32,
    pub range_end: u32,
    /// `underlineStyle`。`"None"` は下線なし（範囲を除外する）。
    pub underline_style: Option<String>,
    /// `underlineThickness`。`"Thick"` はアクティブ変換節、それ以外は細線。
    pub underline_thickness: Option<String>,
}

/// `text` 内の UTF-16 コードユニットオフセットを UTF-8 バイトオフセットへ変換する。
/// EditContext は UTF-16、Hayate core の編集モデルは UTF-8 バイトオフセットを扱うため、
/// 変換節の範囲は core へ渡す前に変換する。負のオフセットは 0 に、末尾超過は末尾にクランプする。
fn utf16_to_byte_offset(text: &str, utf16_offset: i64) -> usize {
    if utf16_offset <= 0 {
        return 0;
    }
    let target = utf16_offset as usize;
    let mut utf16 = 0usize;
    for (byte_idx, ch) in text.char_indices() {
        if utf16 >= target {
            return byte_idx;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

/// EditContext の `textformatupdate` フォーマットを、core が消費する平坦な
/// `[start, end, weight, …]` UTF-8 バイトオフセット三つ組ストリームへ変換する（ADR-0102）。
/// `text` は現在のプリエディット、`base` は EditContext テキスト上の変換中セグメント開始
/// オフセット（UTF-16）。プリエディット外・つぶれた範囲・明示的に下線なしの範囲は除外する。
/// `weight` は太い下線（アクティブ節）で `1`、それ以外は `0`。
pub(crate) fn composition_formats_to_wire(
    text: &str,
    base: u32,
    formats: &[ClauseFormat],
) -> Vec<u32> {
    let mut out = Vec::new();
    for f in formats {
        if f.underline_style.as_deref() == Some("None") {
            continue;
        }
        let start = utf16_to_byte_offset(text, f.range_start as i64 - base as i64);
        let end = utf16_to_byte_offset(text, f.range_end as i64 - base as i64);
        if start >= end {
            continue;
        }
        let weight = u32::from(f.underline_thickness.as_deref() == Some("Thick"));
        out.push(start as u32);
        out.push(end as u32);
        out.push(weight);
    }
    out
}

/// canvas バッキングストアのピクセル矩形 `(x, y, w, h)` を、canvas の CSS バウンディング
/// 矩形 `(left, top, css_w, css_h)` を使ってスクリーン空間の矩形 `(left, top, width, height)` へ
/// 変換する。`EditContext::updateControlBounds` / `updateSelectionBounds` が候補窓を置く座標系。
/// canvas のバッキング幅/高さが 0 のときはスケールを 1 にフォールバックする。
pub(crate) fn canvas_pixel_rect_to_screen(
    css_rect: (f64, f64, f64, f64),
    canvas_width: u32,
    canvas_height: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> (f64, f64, f64, f64) {
    let (left, top, css_w, css_h) = css_rect;
    let scale_x = if canvas_width == 0 {
        1.0
    } else {
        css_w / canvas_width as f64
    };
    let scale_y = if canvas_height == 0 {
        1.0
    } else {
        css_h / canvas_height as f64
    };
    (
        left + x as f64 * scale_x,
        top + y as f64 * scale_y,
        w as f64 * scale_x,
        h as f64 * scale_y,
    )
}

// ── web-sys 配線（wasm32 のみ、薄くテスト対象外。pointer_input に倣う）────────────

/// `render()` 冒頭でドレインする、フレーム間にバッファされた生の編集入力。
/// EditContext / canvas の各リスナがここへ積み、ドレイン時に core の編集シームを駆動する。
/// ターゲット要素はドレイン時に `focused_element()` で解決する（TS が各イベントで
/// `focused_element_id()` を読み直していたのと同型）。
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub(crate) enum EditInput {
    /// `compositionstart`。
    CompositionStart,
    /// 非変換中の `textupdate` → 印字入力。
    Text(String),
    /// 変換中の `textupdate` → preedit 更新（フォーマット無し）。
    CompositionUpdate(String),
    /// `textformatupdate` → 文節下線付き preedit 更新（wire は `[start,end,weight,…]`）。
    CompositionFormat { text: String, wire: Vec<u32> },
    /// `compositionend` → 確定。
    CompositionEnd(String),
    /// canvas の `keydown`（非変換中・編集アーム中のみバッファ）。
    Key { key: String, modifiers: u32 },
}

/// EditContext クロージャ間で共有する変換状態。`textformatupdate` の節範囲を preedit 相対に
/// するための変換中セグメント開始オフセット（UTF-16）と現在の preedit テキストを追う。
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct ComposeState {
    composing: bool,
    base: u32,
    text: String,
}

#[cfg(target_arch = "wasm32")]
use std::cell::{Cell, RefCell};
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{DomRect, Event, EventTarget, HtmlCanvasElement, KeyboardEvent};

// `on_key_down` と共有する `MODIFIER_*` ワイヤビットフィールド（SHIFT=1, CTRL=2, ALT=4, META=8）。
#[cfg(target_arch = "wasm32")]
const MODIFIER_SHIFT: u32 = 1;
#[cfg(target_arch = "wasm32")]
const MODIFIER_CTRL: u32 = 2;
#[cfg(target_arch = "wasm32")]
const MODIFIER_ALT: u32 = 4;
#[cfg(target_arch = "wasm32")]
const MODIFIER_META: u32 = 8;

// プラットフォームの `EditContext` への手書き wasm-bindgen バインディング（ADR-0069）。
// web-sys は EditContext を unstable cfg の裏にしか出さないため、必要な面だけを自前で束ねる
// （codec の js_sys::Reflect interop と同じ流儀）。`EventTarget` を継承し、
// `add_event_listener_with_callback` を再利用する。
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = EventTarget, js_name = EditContext)]
    type EditContext;

    #[wasm_bindgen(constructor, catch)]
    fn new() -> Result<EditContext, JsValue>;

    #[wasm_bindgen(method, js_name = updateControlBounds)]
    fn update_control_bounds(this: &EditContext, rect: &DomRect);

    #[wasm_bindgen(method, js_name = updateSelectionBounds)]
    fn update_selection_bounds(this: &EditContext, rect: &DomRect);
}

#[cfg(target_arch = "wasm32")]
fn reflect_string(obj: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

#[cfg(target_arch = "wasm32")]
fn reflect_u32(obj: &JsValue, key: &str) -> Option<u32> {
    js_sys::Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|n| n as u32)
}

/// `textformatupdate.getTextFormats()` を [`ClauseFormat`] の列へ読み出す。
#[cfg(target_arch = "wasm32")]
fn read_text_formats(event: &JsValue) -> Vec<ClauseFormat> {
    let Ok(getter) = js_sys::Reflect::get(event, &JsValue::from_str("getTextFormats")) else {
        return Vec::new();
    };
    let Ok(getter) = getter.dyn_into::<js_sys::Function>() else {
        return Vec::new();
    };
    let Ok(result) = getter.call0(event) else {
        return Vec::new();
    };
    let arr = js_sys::Array::from(&result);
    let mut out = Vec::new();
    for item in arr.iter() {
        out.push(ClauseFormat {
            range_start: reflect_u32(&item, "rangeStart").unwrap_or(0),
            range_end: reflect_u32(&item, "rangeEnd").unwrap_or(0),
            underline_style: reflect_string(&item, "underlineStyle"),
            underline_thickness: reflect_string(&item, "underlineThickness"),
        });
    }
    out
}

/// `EditContext` が利用可能か（Canvas Mode の前提、ADR-0016）。HTML モードや未対応ブラウザでは
/// グローバルが無いので IME 配線全体をスキップする（旧 `attachTextInput` の早期 return と同型）。
#[cfg(target_arch = "wasm32")]
fn edit_context_supported() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("EditContext"))
        .map(|v| v.is_function())
        .unwrap_or(false)
}

/// 取り付けたリスナを寿命管理し、drop で外す（`PointerInputGuard` と同型）。
#[cfg(target_arch = "wasm32")]
struct ListenerReg {
    target: EventTarget,
    name: &'static str,
    closure: Closure<dyn FnMut(Event)>,
}

/// アダプタが所有する canvas ごとの `EditContext` と、その配線（ADR-0069）。
/// `render()` 末尾で core の `ImePresentation` を反映する: `wants`（=`text-input` フォーカス中）の
/// 間だけ着脱し、候補窓 rect を駆動する。着脱はモバイルのソフトキーボードを表示/解除する。
#[cfg(target_arch = "wasm32")]
pub(crate) struct EditContextHandle {
    canvas: HtmlCanvasElement,
    edit_context: EditContext,
    /// 現在 canvas へアタッチ済みか。冗長な `editContext` セットを避ける。
    attached: Cell<bool>,
    listeners: Vec<ListenerReg>,
}

#[cfg(target_arch = "wasm32")]
impl Drop for EditContextHandle {
    fn drop(&mut self) {
        // デタッチしておく（残った editContext がキーボードを掴んだままにならないように）。
        let _ = js_sys::Reflect::set(
            &self.canvas,
            &JsValue::from_str("editContext"),
            &JsValue::NULL,
        );
        for reg in &self.listeners {
            let _ = reg.target.remove_event_listener_with_callback(
                reg.name,
                reg.closure.as_ref().unchecked_ref(),
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl EditContextHandle {
    /// core が今フレームでキーボードを上げたいか（`wants`）に応じて `EditContext` を着脱する。
    /// 着脱がモバイルのソフトキーボードを表示/解除する（ADR-0069 / #392）。
    pub(crate) fn set_attached(&self, wants: bool) {
        if self.attached.get() == wants {
            return;
        }
        let value: JsValue = if wants {
            JsValue::from(self.edit_context.clone())
        } else {
            JsValue::NULL
        };
        let _ = js_sys::Reflect::set(&self.canvas, &JsValue::from_str("editContext"), &value);
        self.attached.set(wants);
    }

    /// 候補窓を `(x, y, w, h)`（スクリーン空間）へ合わせる。
    pub(crate) fn update_bounds(&self, x: f64, y: f64, w: f64, h: f64) {
        if let Ok(rect) = DomRect::new_with_x_and_y_and_width_and_height(x, y, w, h) {
            self.edit_context.update_control_bounds(&rect);
            self.edit_context.update_selection_bounds(&rect);
        }
    }
}

/// `canvas` に自前で `EditContext` を生成・配線する（ADR-0069 / ADR-0080）。
///
/// 起動時にはアタッチしない（モバイルのソフトキーボードが立つため）。アタッチは
/// `render()` 末尾の同期に委ね、core が `text-input` フォーカスを報告している間だけ着ける。
/// 生のイベントは `pending` バッファへ積み、`render()` 冒頭でドレインする。`edit_armed` は
/// 直近の `render()` が更新する「何か focus 中 or 選択あり」フラグで、keydown ゲートに使う。
/// `EditContext` 非対応（HTML モード等）なら `Ok(None)`。
#[cfg(target_arch = "wasm32")]
pub(crate) fn attach_edit_context(
    canvas: &HtmlCanvasElement,
    pending: Rc<RefCell<Vec<EditInput>>>,
    edit_armed: Rc<RefCell<bool>>,
    request_redraw: Rc<RefCell<Option<js_sys::Function>>>,
) -> Result<Option<EditContextHandle>, JsValue> {
    if !edit_context_supported() {
        return Ok(None);
    }
    let edit_context = EditContext::new()?;
    // canvas が keydown を受けられるよう focusable にする。
    canvas.set_tab_index(0);

    let compose = Rc::new(RefCell::new(ComposeState::default()));
    let mut listeners: Vec<ListenerReg> = Vec::new();
    let ec_target: EventTarget = edit_context.clone().unchecked_into();

    // compositionstart: 変換開始。base に確定済み末尾（selectionStart）を取る。
    {
        let compose = compose.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let ec = edit_context.clone();
        let closure = Closure::wrap(Box::new(move |_event: Event| {
            let mut c = compose.borrow_mut();
            c.composing = true;
            // 確定済み末尾（変換中セグメントの開始）。EditContext のプロパティを直接読む。
            c.base = reflect_u32(&JsValue::from(ec.clone()), "selectionStart").unwrap_or(0);
            c.text = String::new();
            pending.borrow_mut().push(EditInput::CompositionStart);
            drop(c);
            crate::pointer_input::wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        ec_target
            .add_event_listener_with_callback("compositionstart", closure.as_ref().unchecked_ref())?;
        listeners.push(ListenerReg {
            target: ec_target.clone(),
            name: "compositionstart",
            closure,
        });
    }

    // textupdate: 変換中は preedit 更新、そうでなければ印字入力。
    {
        let compose = compose.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let ev: &JsValue = event.as_ref();
            let text = reflect_string(ev, "text").unwrap_or_default();
            let mut c = compose.borrow_mut();
            if c.composing {
                c.base = reflect_u32(ev, "updateRangeStart").unwrap_or(0);
                c.text = text.clone();
                // フォーマット無しの更新を先に送る。変換下線は後続の textformatupdate で届く。
                pending.borrow_mut().push(EditInput::CompositionUpdate(text));
            } else {
                pending.borrow_mut().push(EditInput::Text(text));
            }
            drop(c);
            crate::pointer_input::wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        ec_target
            .add_event_listener_with_callback("textupdate", closure.as_ref().unchecked_ref())?;
        listeners.push(ListenerReg {
            target: ec_target.clone(),
            name: "textupdate",
            closure,
        });
    }

    // textformatupdate: 文節下線。preedit 相対の UTF-8 バイト三つ組へ変換して積む（ADR-0102）。
    {
        let compose = compose.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let c = compose.borrow();
            if !c.composing {
                return;
            }
            let formats = read_text_formats(event.as_ref());
            let wire = composition_formats_to_wire(&c.text, c.base, &formats);
            pending.borrow_mut().push(EditInput::CompositionFormat {
                text: c.text.clone(),
                wire,
            });
            drop(c);
            crate::pointer_input::wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        ec_target.add_event_listener_with_callback(
            "textformatupdate",
            closure.as_ref().unchecked_ref(),
        )?;
        listeners.push(ListenerReg {
            target: ec_target.clone(),
            name: "textformatupdate",
            closure,
        });
    }

    // compositionend: 確定。
    {
        let compose = compose.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let data = reflect_string(event.as_ref(), "data").unwrap_or_default();
            let mut c = compose.borrow_mut();
            c.composing = false;
            c.text = String::new();
            pending.borrow_mut().push(EditInput::CompositionEnd(data));
            drop(c);
            crate::pointer_input::wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        ec_target
            .add_event_listener_with_callback("compositionend", closure.as_ref().unchecked_ref())?;
        listeners.push(ListenerReg {
            target: ec_target.clone(),
            name: "compositionend",
            closure,
        });
    }

    // canvas keydown: 編集キーをバッファする。変換中は IME がキーを所有するので握り潰し、
    // 非印字キーは既定動作（ページスクロール等）を抑制する。`edit_armed` ゲートは
    // 何も focus されておらず選択も無いときにキーを素通しする（旧 attachTextInput と同型）。
    {
        let compose = compose.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let armed = edit_armed.clone();
        let canvas_target: EventTarget = canvas.clone().unchecked_into();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Ok(ke) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if !*armed.borrow() {
                return;
            }
            let key = ke.key();
            // ファンクションキー（F1–F24）はブラウザ/OS 予約。DevTools（F12）や
            // リロード（F5）等を奪わないよう、変換状態・バッファに関係なく素通しする。
            if crate::edit_keymap::is_browser_reserved_key(&key) {
                return;
            }
            if compose.borrow().composing {
                if key != "Escape" {
                    ke.prevent_default();
                }
                return;
            }
            let mut mods = 0u32;
            if ke.shift_key() {
                mods |= MODIFIER_SHIFT;
            }
            if ke.ctrl_key() {
                mods |= MODIFIER_CTRL;
            }
            if ke.alt_key() {
                mods |= MODIFIER_ALT;
            }
            if ke.meta_key() {
                mods |= MODIFIER_META;
            }
            pending.borrow_mut().push(EditInput::Key {
                key: key.clone(),
                modifiers: mods,
            });
            crate::pointer_input::wake(&request_redraw);
            // 印字可能な単一文字（修飾なし）は EditContext の textupdate が挿入を担うため
            // 既定動作を残す。それ以外（矢印・Backspace 等）はページ操作を抑制する。
            let printable =
                key.chars().count() == 1 && !ke.ctrl_key() && !ke.meta_key() && !ke.alt_key();
            if !printable {
                ke.prevent_default();
            }
        }) as Box<dyn FnMut(Event)>);
        canvas_target
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        listeners.push(ListenerReg {
            target: canvas_target,
            name: "keydown",
            closure,
        });
    }

    Ok(Some(EditContextHandle {
        canvas: canvas.clone(),
        edit_context,
        attached: Cell::new(false),
        listeners,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(start: u32, end: u32, thickness: &str) -> ClauseFormat {
        ClauseFormat {
            range_start: start,
            range_end: end,
            underline_style: None,
            underline_thickness: Some(thickness.to_string()),
        }
    }

    #[test]
    fn converts_utf16_clause_ranges_to_utf8_byte_triples_relative_to_the_base() {
        // 未確定文字列 "ぎゅうにゅう": UTF-16 で 6 単位、UTF-8 で 18 バイト（各3バイト）。
        // 変換中セグメントは EditContext オフセット 2（確定済み2文字分）から始まる。
        let wire = composition_formats_to_wire(
            "ぎゅうにゅう",
            2,
            &[fmt(2, 5, "Thick"), fmt(5, 8, "Thin")],
        );
        assert_eq!(wire, vec![0, 9, 1, 9, 18, 0]);
    }

    #[test]
    fn drops_non_underlined_and_collapsed_ranges() {
        let formats = [
            ClauseFormat {
                range_start: 0,
                range_end: 1,
                underline_style: Some("None".to_string()),
                underline_thickness: Some("Thick".to_string()),
            },
            fmt(2, 2, "Thin"), // つぶれた範囲
            fmt(1, 3, "Thin"),
        ];
        let wire = composition_formats_to_wire("abc", 0, &formats);
        assert_eq!(wire, vec![1, 3, 0]);
    }

    #[test]
    fn maps_canvas_pixels_to_css_screen_coordinates() {
        // canvas 200x100、CSS rect が (10,20) 原点で 400x200 → 2倍スケール。
        let rect = canvas_pixel_rect_to_screen((10.0, 20.0, 400.0, 200.0), 200, 100, 50.0, 25.0, 8.0, 16.0);
        assert_eq!(rect, (110.0, 70.0, 16.0, 32.0));
    }

    #[test]
    fn zero_backing_size_falls_back_to_unit_scale() {
        let rect = canvas_pixel_rect_to_screen((0.0, 0.0, 100.0, 50.0), 0, 0, 4.0, 8.0, 2.0, 6.0);
        assert_eq!(rect, (4.0, 8.0, 2.0, 6.0));
    }
}
