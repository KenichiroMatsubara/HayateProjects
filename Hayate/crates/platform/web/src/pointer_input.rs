//! Canvas が自前で配線するポインタ入力（ADR-0080 / ADR-0082）。
//!
//! Canvas の DOM Pointer Events（`pointerdown` / `pointermove` / `pointerup` /
//! `pointerleave` / `pointercancel`）と `wheel` を `attach_pointer_input`（wasm32 のみ）で
//! 購読する。`attach_resize_observer` と同様、リスナの `Closure` はガードで生かし、
//! 順序付き `pending_pointer` バッファへ積んで `render()` 冒頭でドレインする。
//! `pointerleave` / `pointercancel` は座標非依存で、`ElementTree::on_pointer_leave()` /
//! `on_pointer_cancel()`（後者は進行中の押下も終了させる）経由で hover をクリアする。
//! `toCanvas` 座標変換と 1px の移動コアレッシング（leave/cancel によるアンカーリセットを含む）は
//! 純粋関数で、全ターゲットでユニットテストされる。

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use js_sys::Function;
#[cfg(target_arch = "wasm32")]
use web_sys::{
    AddEventListenerOptions, Event, HtmlCanvasElement, MouseEvent, PointerEvent, WheelEvent,
};

/// ADR-0080/0126: 入力がバッファされたら on-demand フレームループを 1 フレーム起こす。
/// `request_redraw` セルは `set_request_redraw` で JS の `scheduleFrame` が注入される。まだ
/// 注入されていなければ no-op（start 前の入力は次の start が拾う）。`scheduleFrame` は冪等
/// なので、同フレーム内に複数入力が来ても二重武装しない。
#[cfg(target_arch = "wasm32")]
pub(crate) fn wake(request_redraw: &Rc<RefCell<Option<Function>>>) {
    if let Some(cb) = request_redraw.borrow().as_ref() {
        let _ = cb.call0(&JsValue::NULL);
    }
}

/// フレーム間でバッファされる生のポインタ入力。座標は DOM イベント捕捉時に
/// `toCanvas` 変換済みで、canvas バッキングストア空間にある。
use hayate_core::PointerKind;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerInput {
    /// `pointerdown`。修飾キービットフィールド（`MODIFIER_*` ワイヤ契約）を運び、
    /// Shift+クリックで選択を拡張できるようにする（ADR-0097）。[`PointerKind`] も運び
    /// （ADR-0082/ADR-0104）、Core が操作ごとにデバイスを保持して touch/pen のみ
    /// drag→scroll 経路に乗せ、mouse は選択/ドラッグを維持する。
    Down {
        x: f32,
        y: f32,
        modifiers: u32,
        kind: PointerKind,
    },
    /// `pointermove`。[`PointerKind`] を運び、Core の `last_pointer_kind` が
    /// セッション途中でもハイブリッドデバイスに追従できるようにする。
    Move { x: f32, y: f32, kind: PointerKind },
    /// `pointerup`。[`PointerKind`] を運ぶ。
    Up { x: f32, y: f32, kind: PointerKind },
    /// ポインタが canvas 面を離れた（`pointerleave`）。座標非依存で、
    /// `ElementTree::on_pointer_leave()` へドレインし hover をクリアする。
    Leave,
    /// `pointercancel`: touch の中断 / pointer-capture の喪失。座標を持たない。
    /// Core の `on_pointer_cancel` は座標非依存（位置に関わらず hover をクリアし
    /// 進行中の押下を終了させる）。
    Cancel,
    Wheel {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
}

/// ビューポート相対のクライアント座標を Hayate の **レイアウト座標**（CSS px）へ
/// 変換する。これは Core のヒットテストと `layout_cache` が住む空間。レイアウト
/// ビューポートは canvas の CSS コンテンツボックスから設定される（`viewport = css_size`）ため、
/// canvas の CSS 原点を引くだけでよく、`devicePixelRatio` のスケーリングは不要。
///
/// ここでバッキングストア（CSS px × dpr）にスケールすると Core に物理ピクセル座標を
/// 渡すことになり、HiDPI ディスプレイでヒットテストを外す（クリックが意図の `dpr×` の位置に着く）。
/// dpr スケールはレンダリング側で `content_scale` により別途適用される（`backend::mod` 参照）。
pub fn to_layout_coords(
    client_x: f32,
    client_y: f32,
    rect_left: f32,
    rect_top: f32,
) -> (f32, f32) {
    (client_x - rect_left, client_y - rect_top)
}

/// 直前に適用した移動から 1px 以内の連続するポインタ移動をまとめる。他の入力の
/// 到着順は保つ。`seed` は前回ドレインで適用した最後の移動位置で、フレーム境界を
/// またぐ微小移動もまとめられる。移動以外の入力はそのまま通過しコアレッシング
/// アンカーを動かさない（Core で `on_pointer_down/up`/wheel が `last_pointer_pos` を
/// 変えないのと一致）。
pub fn coalesce_pointer_inputs(
    inputs: impl IntoIterator<Item = PointerInput>,
    seed: Option<(f32, f32)>,
) -> Vec<PointerInput> {
    let mut anchor = seed;
    let mut out = Vec::new();
    for input in inputs {
        match input {
            PointerInput::Move { x, y, .. } => {
                if let Some((ax, ay)) = anchor {
                    if (x - ax).abs() < 1.0 && (y - ay).abs() < 1.0 {
                        continue;
                    }
                }
                anchor = Some((x, y));
            }
            // leave/cancel は hover をクリアし Core の last_pointer_pos をリセットするので、
            // コアレッシングアンカーもリセットする必要がある。同座標への再入移動を
            // 通過させて `:hover` を再適用するため。
            PointerInput::Leave | PointerInput::Cancel => anchor = None,
            _ => {}
        }
        out.push(input);
    }
    out
}

/// 次回ドレインの種にすべきコアレッシングアンカー。`seed` から始めて同じ
/// move/leave アンカーロジックを `inputs` に再生する。移動はアンカーを設定、leave は
/// クリア、他の入力は据え置く。これにより 1px 重複排除がフレーム境界をまたぎつつ、
/// `pointerleave` を越えて古い位置を漏らさない。
pub fn final_anchor(inputs: &[PointerInput], seed: Option<(f32, f32)>) -> Option<(f32, f32)> {
    let mut anchor = seed;
    for input in inputs {
        match input {
            PointerInput::Move { x, y, .. } => anchor = Some((*x, *y)),
            PointerInput::Leave | PointerInput::Cancel => anchor = None,
            _ => {}
        }
    }
    anchor
}

// ── web-sys 配線（wasm32 のみ、薄くテスト対象外。attach_resize_observer に倣う）

#[cfg(target_arch = "wasm32")]
pub(crate) struct PointerInputGuard {
    canvas: HtmlCanvasElement,
    listeners: Vec<(&'static str, Closure<dyn FnMut(Event)>)>,
}

#[cfg(target_arch = "wasm32")]
impl Drop for PointerInputGuard {
    fn drop(&mut self) {
        for (name, closure) in &self.listeners {
            let _ = self
                .canvas
                .remove_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
        }
    }
}

/// `canvas` に `pointerdown` / `pointermove` / `pointerup` + `wheel` リスナを自前で取り付け、
/// 各入力（座標変換済み）を `pending` へ積む。
#[cfg(target_arch = "wasm32")]
pub(crate) fn attach_pointer_input(
    canvas: &HtmlCanvasElement,
    pending: Rc<RefCell<Vec<PointerInput>>>,
    request_redraw: Rc<RefCell<Option<Function>>>,
) -> Result<PointerInputGuard, JsValue> {
    let mut listeners: Vec<(&'static str, Closure<dyn FnMut(Event)>)> = Vec::new();

    // Canvas Mode が touch スクロールを管理するため、面上のブラウザ既定の
    // touch-action（pan/zoom）を抑制する（ADR-0082 / ADR-0080: Rust アダプタが
    // 自己設定し、ホスト側の接着コードは不要）。
    let _ = canvas.style().set_property("touch-action", "none");

    {
        // `pointerdown` では修飾キーも捕捉して Shift+クリックで選択を拡張できるようにし、
        // `pointerType` も捕捉してドレインが touch/pen をスクロールジェスチャへ振り分けられるようにする。
        // 追跡するのはプライマリポインタのみで、2 本目の指やピンチは無視する。
        // touch/pen では指が canvas を離れてもジェスチャが移動を受け続けるよう
        // ポインタもキャプチャする。
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(pe) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            if !pe.is_primary() {
                return;
            }
            let kind = PointerKind::from_dom(&pe.pointer_type());
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, pe.as_ref());
            let modifiers = mouse_modifiers(pe.as_ref());
            if hayate_core::scroll::is_drag_scroll_pointer(kind) {
                let _ = canvas_for_cb.set_pointer_capture(pe.pointer_id());
            }
            pending.borrow_mut().push(PointerInput::Down {
                x,
                y,
                modifiers,
                kind,
            });
            wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback("pointerdown", closure.as_ref().unchecked_ref())?;
        listeners.push(("pointerdown", closure));
    }

    for (name, make) in [
        ("pointermove", make_move as fn(f32, f32, PointerKind) -> PointerInput),
        ("pointerup", make_up),
    ] {
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(pe) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            // プライマリポインタのみ追跡する（余分な指 / ピンチは無視）。
            if !pe.is_primary() {
                return;
            }
            // デバイスを運び、Core の `last_pointer_kind` が操作を始めた押下だけでなく
            // 現在のポインタに追従するようにする。
            let kind = PointerKind::from_dom(&pe.pointer_type());
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, pe.as_ref());
            pending.borrow_mut().push(make(x, y, kind));
            wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())?;
        listeners.push((name, closure));
    }

    {
        // `pointerleave` は座標非依存で、Core の hover 集合全体をクリアするため
        // `toCanvas` 変換は不要。
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |_event: Event| {
            pending.borrow_mut().push(PointerInput::Leave);
            wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback("pointerleave", closure.as_ref().unchecked_ref())?;
        listeners.push(("pointerleave", closure));
    }

    {
        // `pointercancel` は座標非依存（Core が位置に関わらず hover と active を
        // クリアする）なので、素の `Cancel` を積むだけ。
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            if event.dyn_ref::<PointerEvent>().is_none() {
                return;
            }
            pending.borrow_mut().push(PointerInput::Cancel);
            wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback("pointercancel", closure.as_ref().unchecked_ref())?;
        listeners.push(("pointercancel", closure));
    }

    {
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let request_redraw = request_redraw.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(we) = event.dyn_ref::<WheelEvent>() else {
                return;
            };
            // Canvas Mode が wheel スクロールを端から端まで管理する。`apply_wheel_delta` が
            // ヒットした `scroll-view` を動かし、消費されなかった残りを root まで連鎖させる（ADR-0084）。
            // ブラウザ既定のスクロールを抑制し、canvas 内の wheel が canvas 内スクロールに加えて
            // ページ（やネイティブのスクロール可能な祖先）まで *同時に* スクロールしないようにする。
            // touch の `touch-action: none` に対する wheel 版。これがないと子の scroll-view と
            // 周囲のページが一緒にスクロールしてしまう（二重スクロール）。
            we.prevent_default();
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, we.as_ref());
            pending.borrow_mut().push(PointerInput::Wheel {
                x,
                y,
                delta_x: we.delta_x() as f32,
                delta_y: we.delta_y() as f32,
            });
            wake(&request_redraw);
        }) as Box<dyn FnMut(Event)>);
        // 上の `prevent_default` が実際にネイティブスクロールを抑制するよう非 passive にする。
        // passive リスナだと黙って無視されページもスクロールしてしまう。
        let opts = AddEventListenerOptions::new();
        opts.set_passive(false);
        canvas.add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            closure.as_ref().unchecked_ref(),
            &opts,
        )?;
        listeners.push(("wheel", closure));
    }

    Ok(PointerInputGuard {
        canvas: canvas.clone(),
        listeners,
    })
}

/// mouse/pointer イベントの修飾キーを `on_key_down` と共有する `MODIFIER_*` ワイヤ
/// ビットフィールド（SHIFT=1, CTRL=2, ALT=4, META=8）へ詰める。
#[cfg(target_arch = "wasm32")]
fn mouse_modifiers(event: &MouseEvent) -> u32 {
    let mut mods = 0;
    if event.shift_key() {
        mods |= 1;
    }
    if event.ctrl_key() {
        mods |= 2;
    }
    if event.alt_key() {
        mods |= 4;
    }
    if event.meta_key() {
        mods |= 8;
    }
    mods
}
#[cfg(target_arch = "wasm32")]
fn make_move(x: f32, y: f32, kind: PointerKind) -> PointerInput {
    PointerInput::Move { x, y, kind }
}
#[cfg(target_arch = "wasm32")]
fn make_up(x: f32, y: f32, kind: PointerKind) -> PointerInput {
    PointerInput::Up { x, y, kind }
}

/// `MouseEvent`（やそのサブクラス）から `clientX/clientY` を読み、canvas の現在の
/// CSS バウンディング矩形の原点を使って Hayate のレイアウト座標（CSS px）へ変換する。
#[cfg(target_arch = "wasm32")]
fn pointer_event_to_canvas(canvas: &HtmlCanvasElement, event: &MouseEvent) -> (f32, f32) {
    let rect = canvas.get_bounding_client_rect();
    to_layout_coords(
        event.client_x() as f32,
        event.client_y() as f32,
        rect.left() as f32,
        rect.top() as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesce_preserves_arrival_order_of_distinct_inputs() {
        let inputs = vec![
            PointerInput::Down { x: 10.0, y: 10.0, modifiers: 0, kind: PointerKind::Mouse },
            PointerInput::Move { x: 20.0, y: 20.0, kind: PointerKind::Mouse },
            PointerInput::Up { x: 20.0, y: 20.0, kind: PointerKind::Mouse },
        ];
        let out = coalesce_pointer_inputs(inputs.clone(), None);
        assert_eq!(out, inputs);
    }

    #[test]
    fn coalesce_drops_consecutive_sub_pixel_moves() {
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
            PointerInput::Move { x: 50.4, y: 50.2, kind: PointerKind::Mouse }, // (50,50) の 1px 以内 → 破棄
            PointerInput::Move { x: 60.0, y: 50.0, kind: PointerKind::Mouse }, // 1px 超 → 残す
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
                PointerInput::Move { x: 60.0, y: 50.0, kind: PointerKind::Mouse },
            ]
        );
    }

    #[test]
    fn coalesce_does_not_collapse_moves_across_a_down() {
        // ほぼ同一位置の間にある押下は残らねばならない。down/up はコアレッシング
        // アンカーを動かさないが、順序は保つ必要がある。
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
            PointerInput::Down { x: 50.0, y: 50.0, modifiers: 0, kind: PointerKind::Mouse },
            PointerInput::Move { x: 50.2, y: 50.0, kind: PointerKind::Mouse }, // まだアンカーの 1px 以内 → 破棄
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
                PointerInput::Down { x: 50.0, y: 50.0, modifiers: 0, kind: PointerKind::Mouse },
            ]
        );
    }

    #[test]
    fn coalesce_resets_anchor_on_cancel_so_re_entry_move_survives() {
        // `pointercancel` は（leave と同様に）hover をクリアし Core の last_pointer_pos を
        // リセットするので、コアレッシングアンカーもリセットせねばならない。同座標への
        // 再入移動を通過させて `:hover` を再適用するため。
        let inputs = vec![
            PointerInput::Move { x: 10.0, y: 10.0, kind: PointerKind::Mouse },
            PointerInput::Cancel,
            PointerInput::Move { x: 10.2, y: 10.0, kind: PointerKind::Mouse }, // (10,10) の 1px 以内だがアンカーリセット → 残す
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 10.0, y: 10.0, kind: PointerKind::Mouse },
                PointerInput::Cancel,
                PointerInput::Move { x: 10.2, y: 10.0, kind: PointerKind::Mouse },
            ],
        );
    }

    #[test]
    fn coalesce_uses_seed_to_drop_first_move_across_frame_boundary() {
        // 最初の移動は前回ドレインで適用した位置を繰り返す。
        let inputs = vec![PointerInput::Move { x: 100.0, y: 100.0, kind: PointerKind::Mouse }];
        let out = coalesce_pointer_inputs(inputs, Some((100.0, 100.0)));
        assert!(out.is_empty());
    }

    #[test]
    fn coalesce_resets_anchor_on_leave_so_re_entry_move_survives() {
        // leave はコアレッシングアンカーをクリアする（Core が last_pointer_pos をリセット）ので、
        // 同座標への再入は破棄してはならない。さもないと re-hover が Core に届かず
        // `:hover` が再適用されない。
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
            PointerInput::Leave,
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
        ];
        let out = coalesce_pointer_inputs(inputs.clone(), None);
        assert_eq!(out, inputs);
    }

    #[test]
    fn final_anchor_carries_last_move_and_clears_on_leave() {
        // 最新の移動が次回ドレインのアンカーになる。移動以外の入力はならない。
        let moved = vec![
            PointerInput::Move { x: 10.0, y: 20.0, kind: PointerKind::Mouse },
            PointerInput::Up { x: 10.0, y: 20.0, kind: PointerKind::Mouse },
        ];
        assert_eq!(final_anchor(&moved, None), Some((10.0, 20.0)));

        // 末尾の leave はアンカーをクリアするので、次フレームの再入移動（同座標でも）が
        // 境界をまたいでコアレッシングされない。
        let left = vec![
            PointerInput::Move { x: 10.0, y: 20.0, kind: PointerKind::Mouse },
            PointerInput::Leave,
        ];
        assert_eq!(final_anchor(&left, None), None);

        // 移動も leave もなければ入力 seed を保つ（位置は不変）。
        assert_eq!(final_anchor(&[], Some((5.0, 5.0))), Some((5.0, 5.0)));
    }

    #[test]
    fn to_layout_coords_maps_client_into_css_layout_space() {
        // CSS ボックス原点が (10,10) の canvas 上のクライアント (210,110) は、
        // canvas ローカルの CSS 点 (200, 100) へ写像される。レイアウトとヒットテストと同じ空間。
        let (x, y) = to_layout_coords(210.0, 110.0, 10.0, 10.0);
        assert_eq!((x, y), (200.0, 100.0));
    }

    #[test]
    fn to_layout_coords_does_not_scale_by_device_pixel_ratio() {
        // 回帰防止: レイアウト/ヒットテストは CSS px に住むが、バッキングストアは
        // CSS px × dpr。ポインタ変換は平行移動のみであるべき。dpr でスケールすると
        // （旧 `canvas_width / rect_width` 係数）HiDPI で全クリックが意図の dpr× の位置に着き、
        // hit_test を外し onClick が発火しなかった（Canvas mode、両バックエンド、dpr ≠ 1）。
        // 400 CSS px 幅ボックスの内側 1 CSS px のクライアント点は CSS 1.0 のままで、
        // dpr-2 バッキングバッファが生む 2.0 にはならない。
        let (x, _) = to_layout_coords(1.0, 0.0, 0.0, 0.0);
        assert_eq!(x, 1.0);
    }
}
