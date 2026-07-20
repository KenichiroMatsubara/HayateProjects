use std::collections::{HashMap, HashSet};

use crate::element::caret_geometry::{CaretGeometry, ParleyCaretGeometry};
use crate::element::edit_state::{Direction, EditIntent, Granularity};
use crate::element::event_spec::{event_document_kind, Event};
use crate::element::id::ElementId;
use crate::element::inline_text::{byte_index_at_point, ifc_root};
use crate::element::pointer::PointerKind;
use crate::element::pointer_gesture::{DragMode, PointerGesture, TapPhase};
use crate::element::selection::{
    self, DocumentSelection, Selection, SelectionPoint, MOD_ALT, MOD_CTRL, MOD_PRIMARY, MOD_SHIFT,
};
use crate::element::style::CursorValue;
use crate::element::tree::{ElementTree, TouchScrollIndicator};
use crate::element::visual_invalidation::VisualInvalidationReach;

/// サブピクセルの pointer-move 重複排除しきい値（px、ADR-0066/0088）。直近の
/// 報告位置からどちらの軸でもこの値未満の移動は合流させ、`PointerMove` の発行を
/// 抑える。移動の coalescing をプラットフォーム横断で統一する。
const POINTER_MOVE_DEDUP_PX: f32 = 1.0;

/// スクロールバーつまみドラッグの最小移動量（px、ADR-0110）。ドラッグ軸でこの値
/// 未満の移動は no-op として無視し、サブピクセルのジッタが Scroll Offset を
/// 揺らさないようにする。
const SCROLLBAR_DRAG_MIN_DELTA_PX: f32 = 1e-6;

/// `on_pointer_move` の出力（ADR-0088）。`moved` は 1px dedup で合流されたか
/// レイアウト未準備でスキップされたとき false。`resolved_cursor` はポインタ下の
/// 要素から解決したカーソルで、Platform Adapter がスタイルに触れず OS／ブラウザ
/// カーソルを駆動できる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerMoveResult {
    pub moved: bool,
    pub resolved_cursor: CursorValue,
}

/// ポインタ座標を target へ解決する policy。platform-facing wrapper は物理入力とこの値を
/// 組み立てるだけで、hit-test や release target の解決を別経路で行わない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerRouting {
    /// Canvas のレイアウト済み ElementTree に対して hit-test する。
    CanvasHitTest,
    /// HTML の `event.target` から得た target。`None` は所属しない DOM target を表す。
    HtmlExplicitTarget(Option<ElementId>),
    /// target を解決せず、座標だけを document event stream へ流す。
    CoordinatesOnly,
}

/// [`InteractionIntent`] の閉じた適用結果。adapter は `Consumed` / `Ignored` を解釈し、
/// pointer-move だけは cursor projection を含む [`PointerMoveResult`] を受け取る。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionResult {
    Consumed,
    Ignored,
    PointerMove(PointerMoveResult),
}

/// 直近の入力イベントのモダリティ（ADR-0102）。Chromium の `:focus-visible`
/// ヒューリスティクは最後の操作を基準にする。キーボード操作は次の focus を
/// リング対象にし、ポインタ操作は不要なウィジェット（例: ボタン）でリングを
/// 抑制する。両 Canvas バックエンドが同一にリングを導けるよう core で追跡する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputModality {
    Pointer,
    Keyboard,
}

/// 入力意図の封筒（ADR-0122 決定 2）。pointer / keyboard / accessibility / edit の
/// すべてが同一の閉じた値型に合流する flat dispatch 封筒で、各 adapter はこの値を
/// 構築するだけで `ElementTree::apply_interaction_intent` に流す。
/// `Focus` / `Click` / `Edit(EditIntent)`（#573）/ pointer arm（#572）に加え、
/// `SetValue` / `ScrollToReveal`（accessibility inbound・#575）を並立 arm に持つ。
/// `Edit` arm は既存の `EditIntent`（ADR-0103）を**そのまま内包**し、edit 専用
/// シームの意味を再定義しない。`SetValue` が `String` を運ぶため値全体は `Copy`
/// ではない（`Clone`）。
#[derive(Clone, Debug, PartialEq)]
pub enum InteractionIntent {
    /// `target` をフォーカス要素にする focus 遷移。直前 focus の blur と、`Blur` /
    /// `Focus` イベントの送出を伴う。既に focus 済みなら no-op。
    Focus(ElementId),
    /// `target` に意味的クリックを発行する。座標 `(x, y)` を `Click` イベントに載せ、
    /// 通常のクリックと同様にバブルさせる。
    Click { target: ElementId, x: f32, y: f32 },
    /// `target`（text-input）へ閉じた [`EditIntent`] 語彙の編集を適用する
    /// （ADR-0103 を内包）。幾何依存の縦移動・表示行 Home/End は `Caret Geometry`
    /// seam の裏で純粋計算される（ADR-0122 決定 5）。
    Edit {
        target: ElementId,
        intent: EditIntent,
    },
    /// canvas `(x, y)` での pointer-down（#572）。スクロールバー／ツールバー／ハンドル
    /// 押下の消費判定、hit-test 駆動の active/focus 遷移、drag-mode 分類（selection /
    /// edit / scrollbar thumb の排他）をこの単一 seam に通す。
    PointerDown {
        x: f32,
        y: f32,
        modifiers: u32,
        pointer_kind: PointerKind,
        routing: PointerRouting,
    },
    /// pointer-up（#572）。`explicit_target` は active セッションが無いときの HTML
    /// フォールバック。生きた押下があればリリースで Click を確定する（ADR-0082）。
    PointerUp {
        x: f32,
        y: f32,
        pointer_kind: PointerKind,
        routing: PointerRouting,
    },
    /// canvas `(x, y)` での pointer-move（#572）。hover/cursor 更新と進行中ドラッグの
    /// 駆動を通す。`on_pointer_move` 側で 1px dedup 済み。
    PointerMove {
        x: f32,
        y: f32,
        pointer_kind: PointerKind,
        routing: PointerRouting,
    },
    /// pointer-cancel（タッチ中断／キャプチャ喪失、#572）。active な押下を解除して
    /// 以降のリリースで Click を発火させない。
    PointerCancel,
    /// `target`（text-input）の値を `value` で意味的に置換する（accessibility inbound・
    /// ADR-0098 / #575）。進行中 preedit を確定してから置換し、`TextInput` を発火する。
    /// `text-input` 以外では no-op。AccessKit `SetValue` と編集経路が同一 seam を共有する。
    SetValue { target: ElementId, value: String },
    /// 最寄りの祖先 `scroll-view` の Scroll Offset を調整して `target` を表示に入れる
    /// （AccessKit `ScrollIntoView`・ADR-0098 / #575）。reveal 幾何（`scroll_axis_to_reveal`）
    /// は seam の裏（`Interaction` 実装）に置き、adapter は intent を出すだけ。
    ScrollToReveal { target: ElementId },
}

/// 横断的 interaction state。実際の適用は private `InteractionSession` が `ElementTree`
/// の排他的借用の内側で document 能力とともに行う。focus に加え、pointer 横断 state
/// （hover / active / press 位置 / `PointerGesture` / modality / pointer-pos / cursor /
/// touch scroll）を所有する（#572）。element 位相・layout 幾何・per-element
/// `EditState`・scroll offset は所有せず、session が実 tree から借りる。
pub(crate) struct Interaction {
    /// 現在フォーカスされている要素。`render` のカーソル点滅・`:focus-visible`・
    /// accessibility outbound が読む唯一の focus 真実。
    pub(crate) focused_element: Option<ElementId>,
    /// 直近入力イベントのモダリティ。ネイティブフォーカスリングの `:focus-visible`
    /// 判定を駆動する（ADR-0102）。
    pub(crate) last_input_modality: InputModality,
    /// 直近ポインタ操作の物理デバイス。インタラクションごとに保持する。
    /// `last_input_modality` とは独立した軸で、タッチ押下は `InputModality::Pointer`
    /// かつ `PointerKind::Touch` になる。
    pub(crate) last_pointer_kind: PointerKind,
    /// CSS `:hover` に一致する要素（ポインタ下の自身または子孫）。
    pub(crate) hovered_elements: HashSet<ElementId>,
    /// 現在押下中（`:active`）の要素。クリックはリリースで確定する（ADR-0082）。
    pub(crate) active_element: Option<ElementId>,
    /// 現在の押下（`active_element`）が始まった canvas 位置。pointer-up の `Click` に
    /// 載せる。押下が（スクロール乗っ取り等で）キャンセルされると `active_element`
    /// とともにクリアされ、以降のリリースはクリックを発火しない。
    pub(crate) active_press_pos: Option<(f32, f32)>,
    /// ポインタジェスチャ分類器（ADR-0066）。進行中のドラッグ種別（読み取り専用
    /// 選択／編集選択／スクロールバーつまみは排他）と、単語／段落ジェスチャ用の
    /// マルチクリック追跡を単独所有する。
    pub(crate) pointer_gesture: PointerGesture,
    /// 前回 render 以降に Touch モダリティでスクロールし、一時インジケータを
    /// 再表示すべき ScrollView（ADR-0110）。
    pub(crate) touch_scroll_pending: HashSet<ElementId>,
    /// ScrollView をキーにした稼働中の Touch 一時インジケータ。
    pub(crate) touch_scroll_indicators: HashMap<ElementId, TouchScrollIndicator>,
    /// サブピクセル move の重複排除用の直近ポインタ位置（ADR-0066）。
    pub(crate) last_pointer_pos: Option<(f32, f32)>,
    /// ポインタ下で直近に解決したカーソル。合成 move で報告する（ADR-0088）。
    pub(crate) last_cursor: CursorValue,
    /// 文書全体で唯一のテキスト選択を所有する deep module（ADR-0097 / #574）。
    /// 正規化・縮退・one-per-document・contains clamp の不変条件を interface の裏で
    /// 守る。`Interaction` が主たる mutator で、read 経路は interface 越しに borrow する。
    pub(crate) selection: DocumentSelection,
    /// フローティングツールバーの ⋮ オーバーフロー副メニューが開いているか
    /// （ADR-0097）。⋮ トグルの押下で開閉し、選択が変われば閉じる。ツールバーは
    /// `selection_toolbar` で都度レイアウトし直すので、この開閉状態だけを保持する。
    pub(crate) toolbar_overflow_open: bool,
    /// リリース済み慣性スクロールの進行中アニメーション（ADR-0082 / ADR-0113 /
    /// ADR-0126）。`(scroll_view, (vx, vy))` — ロックした ScrollView と、オフセット空間
    /// （px/ms）の減衰速度。`render` が毎フレーム `scroll_motion_step` で積分し
    /// （範囲内は摩擦、オーバースクロール中はばね戻し）、静止すると `None` に戻る。
    /// 物理は Core が所有し（`rubber_band_offset` 等と同じく `scroll` モジュールの純関数）、
    /// Platform Adapter は pointer-up で推定した解放速度を `start_scroll_momentum` に
    /// 渡すだけの薄い配線に徹する。継続中は `has_pending_visual_work` を true に保ち、
    /// on-demand フレームループ（ADR-0126）が指を離した直後に idle へ落ちて慣性を
    /// 1 フレームで殺すのを防ぐ。
    pub(crate) scroll_momentum: Option<(ElementId, (f32, f32))>,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            focused_element: None,
            // 最初のキーボードイベントまで Pointer。未フォーカス/ポインタ駆動直後の
            // UI がボタンに余計なリングを出さないように。
            last_input_modality: InputModality::Pointer,
            // 最初の実ポインタイベントがデバイスを報告するまで Mouse。
            last_pointer_kind: PointerKind::Mouse,
            hovered_elements: HashSet::new(),
            active_element: None,
            active_press_pos: None,
            pointer_gesture: PointerGesture::default(),
            touch_scroll_pending: HashSet::new(),
            touch_scroll_indicators: HashMap::new(),
            last_pointer_pos: None,
            last_cursor: CursorValue::Default,
            selection: DocumentSelection::default(),
            toolbar_overflow_open: false,
            scroll_momentum: None,
        }
    }
}

/// Private session for a single intent application. It keeps the borrow local:
/// callers only expose `ElementTree::apply_interaction_intent`, while this module
/// coordinates the tree's interaction state and document capabilities together.
pub(crate) struct InteractionSession<'a> {
    tree: &'a mut ElementTree,
}

impl InteractionSession<'_> {
    pub(crate) fn new(tree: &mut ElementTree) -> InteractionSession<'_> {
        InteractionSession { tree }
    }

    fn apply_intent(&mut self, intent: InteractionIntent) -> InteractionResult {
        match intent {
            InteractionIntent::Focus(id) => {
                self.transition_focus(id);
                InteractionResult::Consumed
            }
            InteractionIntent::Click { target, x, y } => {
                self.tree.emit_interaction(Event::Click {
                    target_id: target,
                    x,
                    y,
                });
                InteractionResult::Consumed
            }
            InteractionIntent::Edit { target, intent } => self
                .tree
                .apply_edit_intent(target, intent)
                .then_some(InteractionResult::Consumed)
                .unwrap_or(InteractionResult::Ignored),
            InteractionIntent::PointerDown {
                x,
                y,
                modifiers,
                pointer_kind,
                routing,
            } => {
                self.tree.interaction.last_pointer_kind = pointer_kind;
                self.tree.interaction.scroll_momentum = None;
                match routing {
                    PointerRouting::CanvasHitTest => {
                        self.tree.dispatch_canvas_pointer_down(x, y, modifiers)
                    }
                    PointerRouting::HtmlExplicitTarget(target) => {
                        self.tree.pointer_down_on_target(target, x, y)
                    }
                    PointerRouting::CoordinatesOnly => self.tree.pointer_down_on_target(None, x, y),
                }
                InteractionResult::Consumed
            }
            InteractionIntent::PointerUp {
                x,
                y,
                pointer_kind,
                routing,
            } => {
                self.tree.interaction.last_pointer_kind = pointer_kind;
                let target = match routing {
                    PointerRouting::CanvasHitTest => self.tree.hit_test(x, y),
                    PointerRouting::HtmlExplicitTarget(target) => target,
                    PointerRouting::CoordinatesOnly => None,
                };
                self.pointer_up(target, x, y);
                InteractionResult::Consumed
            }
            InteractionIntent::PointerMove {
                x,
                y,
                pointer_kind,
                routing,
            } => {
                self.tree.interaction.last_pointer_kind = pointer_kind;
                InteractionResult::PointerMove(self.pointer_move(x, y, routing))
            }
            InteractionIntent::PointerCancel => {
                self.tree.apply_pointer_hover(None);
                self.pointer_cancel();
                InteractionResult::Consumed
            }
            InteractionIntent::SetValue { target, value } => {
                self.tree.apply_semantic_set_value(target, &value);
                InteractionResult::Consumed
            }
            InteractionIntent::ScrollToReveal { target } => {
                self.tree.scroll_into_view(target);
                InteractionResult::Consumed
            }
        }
    }

    fn transition_focus(&mut self, id: ElementId) {
        if self.tree.interaction.focused_element == Some(id) {
            return;
        }
        if let Some(previous) = self.tree.interaction.focused_element {
            self.blur_with_events(previous);
        }
        self.element_focus(id);
        self.tree.emit_interaction(Event::Focus { target_id: id });
    }

    pub(crate) fn element_focus(&mut self, id: ElementId) {
        if self.tree.interaction.focused_element == Some(id) {
            return;
        }
        if let Some(previous) = self.tree.interaction.focused_element {
            self.apply_blur_effects(previous);
        }
        self.apply_focus_effects(id);
        self.tree.interaction.focused_element = Some(id);
    }

    pub(crate) fn element_blur(&mut self, id: ElementId) {
        if self.tree.interaction.focused_element != Some(id) {
            return;
        }
        self.apply_blur_effects(id);
        self.tree.interaction.focused_element = None;
    }

    fn blur_with_events(&mut self, id: ElementId) {
        if self.tree.interaction.focused_element != Some(id) {
            return;
        }
        self.element_blur(id);
        self.commit_preedit_on_blur(id);
        if self.tree.interaction.last_pointer_kind == PointerKind::Touch {
            self.tree.collapse_edit_selection_of(id);
        }
        self.tree.emit_interaction(Event::Blur { target_id: id });
    }

    fn apply_focus_effects(&mut self, id: ElementId) {
        if let Some(element) = self.tree.elements.get_mut(&id) {
            element.cursor_visible = true;
        }
        self.tree
            .engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        self.tree
            .mark_pseudo_activation_dirty(id, crate::element::pseudo_state::PseudoState::Focus);
        self.tree.layout.last_cursor_toggle_ms = None;
    }

    fn apply_blur_effects(&mut self, id: ElementId) {
        if let Some(element) = self.tree.elements.get_mut(&id) {
            element.cursor_visible = false;
        }
        self.tree
            .engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        self.tree
            .mark_pseudo_activation_dirty(id, crate::element::pseudo_state::PseudoState::Focus);
        self.tree.layout.last_cursor_toggle_ms = None;
    }

    fn set_active(&mut self, next: Option<ElementId>) {
        if self.tree.interaction.active_element == next {
            return;
        }
        if let Some(previous) = self.tree.interaction.active_element {
            self.tree.mark_pseudo_activation_dirty(
                previous,
                crate::element::pseudo_state::PseudoState::Active,
            );
        }
        if let Some(current) = next {
            self.tree.mark_pseudo_activation_dirty(
                current,
                crate::element::pseudo_state::PseudoState::Active,
            );
        }
        self.tree.interaction.active_element = next;
        if next.is_none() {
            self.tree.interaction.active_press_pos = None;
        }
    }

    fn pointer_up(&mut self, explicit_target: Option<ElementId>, x: f32, y: f32) {
        let active = self.tree.interaction.active_element;
        if let Some(target_id) = active.or(explicit_target) {
            self.tree.emit_interaction(Event::PointerUp {
                target_id,
                x,
                y,
                pointer_kind: self.tree.interaction.last_pointer_kind,
            });
        }
        if let Some(target_id) = active {
            let (press_x, press_y) = self.tree.interaction.active_press_pos.unwrap_or((0.0, 0.0));
            self.tree.emit_interaction(Event::Click {
                target_id,
                x: press_x,
                y: press_y,
            });
        }
        if let Some(target_id) = active.or(explicit_target) {
            self.tree.emit_interaction(Event::ActiveEnd { target_id });
        }
        self.set_active(None);
        self.tree.interaction.pointer_gesture.end_drag();
    }

    fn pointer_cancel(&mut self) {
        self.tree.interaction.last_pointer_pos = None;
        self.tree.interaction.pointer_gesture.end_drag();
        if let Some(target_id) = self.tree.interaction.active_element {
            self.tree.emit_interaction(Event::ActiveEnd { target_id });
        }
        self.set_active(None);
    }

    fn pointer_move(&mut self, x: f32, y: f32, routing: PointerRouting) -> PointerMoveResult {
        if matches!(routing, PointerRouting::CanvasHitTest) && !self.tree.has_layout() {
            return PointerMoveResult {
                moved: false,
                resolved_cursor: self.tree.interaction.last_cursor,
            };
        }
        if let Some((last_x, last_y)) = self.tree.interaction.last_pointer_pos {
            if (x - last_x).abs() < POINTER_MOVE_DEDUP_PX
                && (y - last_y).abs() < POINTER_MOVE_DEDUP_PX
            {
                return PointerMoveResult {
                    moved: false,
                    resolved_cursor: self.tree.interaction.last_cursor,
                };
            }
        }
        self.tree.interaction.last_pointer_pos = Some((x, y));
        self.tree.push_event(Event::PointerMove {
            x,
            y,
            pointer_kind: self.tree.interaction.last_pointer_kind,
        });
        if matches!(routing, PointerRouting::CanvasHitTest) {
            if let Some(target_id) = self.tree.interaction.active_element {
                self.tree.emit_interaction(Event::PointerDrag {
                    target_id,
                    x,
                    y,
                    pointer_kind: self.tree.interaction.last_pointer_kind,
                });
            }
            let hit = self.tree.hit_test(x, y);
            self.tree.apply_pointer_hover(hit);
            self.tree.interaction.last_cursor = self.tree.resolve_cursor(hit);
            self.drive_active_drag(x, y);
        }
        PointerMoveResult {
            moved: true,
            resolved_cursor: self.tree.interaction.last_cursor,
        }
    }

    fn drive_active_drag(&mut self, x: f32, y: f32) {
        match self.tree.interaction.pointer_gesture.drag() {
            DragMode::Scrollbar(drag) => {
                if let Some(updated) = self.tree.drag_scrollbar_step(drag, x, y) {
                    self.tree
                        .interaction
                        .pointer_gesture
                        .begin_drag(DragMode::Scrollbar(updated));
                }
            }
            DragMode::Edit(input) => self.tree.extend_edit_drag_to(input, x, y),
            DragMode::Selection => {
                if let Some(point) = self.tree.selection_point_at(x, y) {
                    self.extend_selection_focus(point);
                }
            }
            DragMode::None => {}
        }
    }

    fn extend_selection_focus(&mut self, point: SelectionPoint) {
        let Some(previous) = self.tree.interaction.selection.get() else {
            return;
        };
        if previous.focus == point
            || !self
                .tree
                .selection_region_of(point.element)
                .eq(&self.tree.selection_region_of(previous.anchor.element))
        {
            return;
        }
        self.tree.interaction.selection.extend_focus(point);
        if let Some(current) = self.tree.interaction.selection.get() {
            self.tree.mark_selection_dirty(previous);
            self.tree.mark_selection_dirty(current);
            self.tree.emit_interaction(Event::SelectionChange);
        }
    }

    fn commit_preedit_on_blur(&mut self, id: ElementId) {
        let committed = self
            .tree
            .elements
            .get_mut(&id)
            .filter(|element| element.kind == crate::element::kind::ElementKind::TextInput)
            .and_then(|element| element.edit.as_mut())
            .is_some_and(|edit| {
                if edit.preedit.is_some() {
                    edit.commit_preedit();
                    true
                } else {
                    false
                }
            });
        if committed {
            self.tree.emit_interaction(Event::TextInput {
                target_id: id,
                text: self.tree.element_get_text_content(id),
            });
        }
    }
}

/// 進行中の Mouse/Pen スクロールバー・サムドラッグ（ADR-0110）。サム上の
/// pointer-down で捕捉し `on_pointer_move` が駆動する。各移動で軸方向の移動量を
/// Scroll Offset デルタに変換し、ホイールと同じ `apply_wheel_delta` 継ぎ目で
/// コミットする（軸端に達した余りは祖先 ScrollView へ連鎖する）。
///
/// `PointerGesture` の状態として tree 内で保持される。フィールド型はすべて公開型。
#[derive(Clone, Copy, Debug)]
pub struct ScrollbarDrag {
    /// サムをドラッグしている ScrollView。
    pub scroll_view: ElementId,
    /// サムが滑る軸。
    pub axis: crate::element::scene_build::ScrollAxis,
    /// ドラッグ軸上の直近ポインタ座標（canvas 空間）。
    pub last_pos: f32,
    /// トラック px あたりの offset px。`max_offset / thumb_travel` を grab 時に
    /// 取得し、サムがトラック空間でポインタを 1:1 で追従する。
    pub offset_per_px: f32,
}

impl ElementTree {
    /// スクロール競合の解決まで semantic pointer-down を保留する touch 接触を開始する。
    /// pointer modality を記録して進行中の慣性を止めるが、hit-test／`:active`／focus は
    /// 変更しない。Platform Adapter は tap と確定した時点で通常の pointer-down を送る。
    pub fn prepare_deferred_pointer_down(&mut self, kind: PointerKind) {
        self.interaction.last_pointer_kind = kind;
        self.interaction.scroll_momentum = None;
    }

    /// canvas 座標でのポインタダウン（ヒットテスト駆動）。
    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.on_pointer_down_with(x, y, 0);
    }

    /// キーボード修飾と物理 [`PointerKind`] を伴うポインタダウン。Platform Adapter
    /// が DOM `PointerEvent.pointerType` を転送し、Core は操作ごとに保持する
    /// (`last_pointer_kind`)。選択／active 挙動は
    /// [`on_pointer_down_with`](Self::on_pointer_down_with) と同一。
    pub fn on_pointer_down_with_kind(&mut self, x: f32, y: f32, modifiers: u32, kind: PointerKind) {
        let _ = self.apply_interaction_intent(InteractionIntent::PointerDown {
            x,
            y,
            modifiers,
            pointer_kind: kind,
            routing: PointerRouting::CanvasHitTest,
        });
    }

    /// キーボード修飾を伴うポインタダウン（ADR-0097）。Shift は新規選択を始めず
    /// 現在の選択の focus を拡張する。
    pub fn on_pointer_down_with(&mut self, x: f32, y: f32, modifiers: u32) {
        self.on_pointer_down_with_kind(x, y, modifiers, self.interaction.last_pointer_kind);
    }

    /// pointer-down の hit-test／消費判定／begin パイプライン本体（#572）。
    /// `apply_interaction_intent` が mem-take せず直接呼ぶので、`self.interaction.
    /// pointer_gesture` 等を直読みでき、挙動は移行前と同一。
    fn dispatch_canvas_pointer_down(&mut self, x: f32, y: f32, modifiers: u32) {
        // Mouse/Pen スクロールバー（サム／トラック）上の押下はそれを操作して
        // ジェスチャを消費する。オーバーレイ chrome はコンテンツの上にあるため、
        // その押下がコンテンツの選択／focus に届くことはない（ADR-0110）。
        if self.begin_scrollbar_gesture(x, y) {
            return;
        }
        // フローティングツールバーのボタン押下はそのアクションを実行してジェスチャ
        // を消費するので、キャレットを動かさず選択も消さない（ADR-0097）。
        if self.try_selection_toolbar_tap(x, y) {
            return;
        }
        // 選択ドラッグハンドル上の押下はその端点を掴み、ドラッグ選択と同じ
        // active-session キャプチャに乗る（ADR-0097）ので、新規キャレットを落とさず
        // 範囲を調整する。
        if self.begin_handle_drag(x, y) {
            return;
        }
        let hit = self.hit_test(x, y);
        self.pointer_down_on_target(hit, x, y);
        // text-input 内の押下はその編集選択を駆動し（ADR-0097）、下記の読み取り専用
        // SelectionArea 経路に優先する。
        if self.begin_edit_selection(hit, x, y, modifiers) {
            return;
        }
        // 選択ドラッグは同じ active-session キャプチャに乗る（ADR-0097）。
        // Selection Region 内の押下は選択をキャレットへ畳み、ダブル／トリプル押下で
        // 単語／段落へ拡張、Shift で focus を拡張する。
        self.begin_selection_at(x, y, modifiers);
    }

    /// canvas `(x, y)` での長押し。読み取り専用の単語選択を始め、ドラッグハンドル
    /// ＋フローティングツールバーを出すモバイルジェスチャ（ADR-0097）。Platform
    /// Adapter は生の長押しを報告し（タイミングは OS のジェスチャ認識器が持つ。
    /// ダブルタップのタイミングが OS 由来なのと同じ）、core は*何をするか*を持つ。
    /// `selectable` サブツリー外の押下は何も選択しない。text-input の編集選択は
    /// 先に消す（文書全体で単一 active）。
    pub fn on_long_press(&mut self, x: f32, y: f32) {
        // 長押しはタッチジェスチャで、始まる選択は Touch モダリティの操作なので、
        // その chrome（ハンドル＋ツールバー）が出る（ADR-0104）。
        self.interaction.last_pointer_kind = PointerKind::Touch;
        let Some(point) = self.selection_point_at(x, y) else {
            return;
        };
        self.collapse_edit_selections();
        self.select_bounds_at(point, selection::word_bounds);
        // 新規ジェスチャ。続くタップはこの長押しからのマルチクリック周期を再開せず、
        // キャレットを落とすべき。
        self.interaction.pointer_gesture.clear_selection_drag();
        self.interaction.pointer_gesture.reset_taps();
    }

    /// 明示ターゲットへのポインタダウン（HTML Mode）。
    pub fn on_pointer_down_on(&mut self, target: ElementId, x: f32, y: f32) {
        let _ = self.apply_interaction_intent(InteractionIntent::PointerDown {
            x,
            y,
            modifiers: 0,
            pointer_kind: self.interaction.last_pointer_kind,
            routing: PointerRouting::HtmlExplicitTarget(Some(target)),
        });
    }

    fn pointer_down_on_target(&mut self, target: Option<ElementId>, x: f32, y: f32) {
        self.interaction.last_input_modality = InputModality::Pointer;
        if let Some(t) = target {
            self.emit_interaction(Event::PointerDown {
                target_id: t,
                x,
                y,
                pointer_kind: self.interaction.last_pointer_kind,
            });
            // クリックはリリースで確定する（ADR-0082）。押下では `:active` を点け、
            // タップ起点を覚えるだけにする。押下→slop 超過でスクロールに化けた場合は
            // アダプタが `on_pointer_cancel` で押下を解除し、リリースでクリックを
            // 発火させない。ここで Click を出すと、押下した瞬間にクリックが配信され、
            // スクロール中のボタン接触が押下扱いになってしまう。
            self.emit_interaction(Event::ActiveStart { target_id: t });
            // active 状態の設定は同一操作で遷移の切替前ビジュアルを捕捉し `:active`
            // 無効化を記録する（ADR-0100）ので、未 active の見た目が遷移の起点に
            // なる（ADR-0089）。
            self.set_active_element(Some(t));
            // リリースの Click が押下座標を載せられるよう起点を覚える（active を
            // セットした後に書く。`set_active_element(None)` 等の途中クリアに
            // 上書きされないため）。
            self.interaction.active_press_pos = Some((x, y));
            self.transition_focus(t);
        } else if let Some(prev) = self.interaction.focused_element {
            self.blur_with_events(prev);
        }
    }

    /// ポインタアップ。`explicit_target` は active セッションが無いときだけ使う。
    pub fn on_pointer_up(&mut self, x: f32, y: f32) {
        let _ = self.apply_interaction_intent(InteractionIntent::PointerUp {
            x,
            y,
            pointer_kind: self.interaction.last_pointer_kind,
            routing: PointerRouting::CanvasHitTest,
        });
    }

    /// 物理 [`PointerKind`] を伴うポインタアップ。操作ごとに保持する。リリース挙動
    /// は [`on_pointer_up`](Self::on_pointer_up) と同一。
    pub fn on_pointer_up_with_kind(&mut self, x: f32, y: f32, kind: PointerKind) {
        let _ = self.apply_interaction_intent(InteractionIntent::PointerUp {
            x,
            y,
            pointer_kind: kind,
            routing: PointerRouting::CanvasHitTest,
        });
    }

    /// 明示フォールバックターゲットを伴うポインタアップ（HTML Mode）。
    pub fn on_pointer_up_on(&mut self, explicit_target: Option<ElementId>, x: f32, y: f32) {
        let _ = self.apply_interaction_intent(InteractionIntent::PointerUp {
            x,
            y,
            pointer_kind: self.interaction.last_pointer_kind,
            routing: PointerRouting::HtmlExplicitTarget(explicit_target),
        });
    }

    /// ポインタキャンセル（タッチ中断／ポインタキャプチャ喪失）。座標非依存で、
    /// hover 集合全体をクリアし（離脱した各要素に `HoverLeave` を発行、保存した
    /// 最終ポインタ位置をリセット。surface-leave の hover クリアと同一）、加えて
    /// active な押下を終える（`active_element.take()` → `ActiveEnd` ＋擬似活性
    /// dirty。pointer-up 経路を写す）。`PointerMove` は捏造しない。
    pub fn on_pointer_cancel(&mut self) {
        self.apply_interaction_intent(InteractionIntent::PointerCancel);
    }

    /// 物理 [`PointerKind`] を伴うポインタムーブ。操作ごとに保持し、発行される
    /// `PointerMove` ワイヤイベントと `last_pointer_kind` が現デバイスを反映する
    /// （ハイブリッド機はセッション途中で切り替わる）。hover／cursor 挙動は
    /// [`on_pointer_move`](Self::on_pointer_move) と同一。
    pub fn on_pointer_move_with_kind(
        &mut self,
        x: f32,
        y: f32,
        kind: PointerKind,
    ) -> PointerMoveResult {
        match self.apply_interaction_intent(InteractionIntent::PointerMove {
            x,
            y,
            pointer_kind: kind,
            routing: PointerRouting::CanvasHitTest,
        }) {
            InteractionResult::PointerMove(result) => result,
            _ => unreachable!("pointer move always returns a PointerMoveResult"),
        }
    }

    /// レイアウトガードと 1px dedup 付きのポインタムーブ。合流時 `moved` は false。
    /// `resolved_cursor` はポインタ下の要素から解決したカーソル（ADR-0088）で、
    /// 合流した移動では不変のまま持ち越す。
    pub fn on_pointer_move(&mut self, x: f32, y: f32) -> PointerMoveResult {
        self.on_pointer_move_with_kind(x, y, self.interaction.last_pointer_kind)
    }

    /// ポインタ下の要素の実効カーソルを「明示 `cursor` → 要素種別の UA 既定 →
    /// `Default`」の順で解決する（ADR-0105）。ブラウザの UA スタイルシートを写す。
    /// 祖先チェーン上のどこかの明示 `cursor` が常に勝つ（CSS `cursor` は継承）。
    /// 未設定のときだけ種別既定が効く。`text-input` と `selectable` テキストは
    /// `text`（I-beam）、`button` は `pointer` に解決する。チェーンに何も寄与が
    /// 無いかポインタが何にも当たらないとき `Default`。
    fn resolve_cursor(&self, hit: Option<ElementId>) -> CursorValue {
        // パス1: ヒット要素か祖先の明示 `cursor` が勝つ。
        let mut current = hit;
        while let Some(id) = current {
            let Some(el) = self.elements.get(&id) else {
                break;
            };
            if let Some(cursor) = el.visual.cursor {
                return cursor;
            }
            current = el.parent;
        }
        // パス2: 明示カーソルが無ければ要素種別の UA 既定へフォールバック。上へ
        // たどることで、種別／selectable 領域の既定が内部に描かれるテキストや子
        // 要素にも届く。
        let mut current = hit;
        while let Some(id) = current {
            let Some(el) = self.elements.get(&id) else {
                break;
            };
            let kind_default = el.kind.default_cursor();
            if kind_default != CursorValue::Default {
                return kind_default;
            }
            // selectable でテキストを持つ要素は text（I-beam）に解決する。選択可能
            // テキストの UA 既定（ADR-0105）。`selectable` な Selection Region 根では
            // なく実効 `user-select`（ADR-0108）を基準にするので、種別既定の
            // `user-select: text` を持つ素の段落は明示領域なしで I-beam を出す。
            // テキストを持つ種別に限定するので、空の `view`（同じく種別既定 `text`）
            // は矢印のまま。ブラウザのテキスト限定 I-beam に合わせる。
            if el.kind.is_text_like()
                && el.user_select == crate::element::style::UserSelectValue::Text
                && !self.user_select_excludes(id)
            {
                return CursorValue::Text;
            }
            current = el.parent;
        }
        CursorValue::Default
    }

    /// ターゲットなしのポインタムーブ（ヒットテスト hover を伴わない HTML Mode
    /// 座標ストリーム）。
    pub fn on_pointer_move_coords(&mut self, x: f32, y: f32) -> bool {
        matches!(
            self.apply_interaction_intent(InteractionIntent::PointerMove {
                x,
                y,
                pointer_kind: self.interaction.last_pointer_kind,
                routing: PointerRouting::CoordinatesOnly,
            }),
            InteractionResult::PointerMove(PointerMoveResult { moved: true, .. })
        )
    }

    /// ポインタが surface を離れた（座標非依存）。hover 集合全体をクリアし
    /// （離脱した各要素に `HoverLeave` を発行、擬似活性 dirty を記録）、再入が
    /// 合流で消されないよう保存した最終ポインタ位置をリセットする。幻の
    /// `PointerMove` は push しない。HTML adapter の要素別 leave 継ぎ目と対称。
    pub fn on_pointer_leave(&mut self) {
        self.apply_pointer_hover(None);
        self.interaction.last_pointer_pos = None;
    }

    pub fn on_wheel(&mut self, target: ElementId, delta_x: f32, delta_y: f32) {
        self.emit_interaction(Event::Scroll {
            target_id: target,
            delta_x,
            delta_y,
        });
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.set_viewport(width, height);
        self.push_event(Event::Resize { width, height });
    }

    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        // キーボード操作は Chromium の `:focus-visible` ヒューリスティクで次の
        // focus をリング対象にする。早期 return より前に記録するので、focus 中の
        // 要素に届かないキー押下でもモダリティは切り替わる。
        self.interaction.last_input_modality = InputModality::Keyboard;
        // 選択キーボードジェスチャは文書全体の選択に作用し要素 focus に依存しない
        // ので、先に走り、適用時にキーを消費する（例: Ctrl/Cmd+A、SelectionArea 上の
        // Shift+Arrow）。
        if self.handle_selection_key(key, modifiers) {
            return;
        }
        let Some(focused) = self.interaction.focused_element else {
            return;
        };
        self.emit_interaction(Event::KeyDown {
            target_id: focused,
            key: key.to_string(),
            modifiers,
        });
    }

    pub fn on_text_input(&mut self, target: ElementId, text: &str) {
        let edited = if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            // キャレット位置に挿入し、選択範囲があれば置換する（ADR-0097）。
            edit.insert(text);
            true
        } else {
            false
        };
        // 挿入は a11y value（`edit.display_text()`）を変える。編集は視覚を別経路で反映し dirty 集合を
        // 通らないため、a11y 世代を明示的に進めて次の poll が最新値を返すようにする（#642）。
        if edited {
            self.bump_a11y_generation();
        }
        // input イベントの value は要素の現在値全体（DOM の `input` → `target.value` と同型）。
        // 挿入された断片ではなく結合表示テキストを載せることで、web ホストは
        // `element_get_text_content` の読み戻し無しに配信ペイロードをそのまま使える（ADR-0069 / #474）。
        let value = self.element_get_text_content(target);
        self.emit_interaction(Event::TextInput {
            target_id: target,
            text: value,
        });
    }

    pub fn on_composition_start(&mut self, target: ElementId, text: &str) {
        let edited = if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit(text);
            true
        } else {
            false
        };
        // preedit は display_text に含まれる＝a11y value を変える（#642）。
        if edited {
            self.bump_a11y_generation();
        }
        self.emit_interaction(Event::CompositionStart {
            target_id: target,
            text: text.to_string(),
        });
    }

    pub fn on_composition_update(&mut self, target: ElementId, text: &str) {
        self.on_composition_update_formatted(target, text, Vec::new());
    }

    /// EditContext `textformatupdate` の文節フォーマット範囲を伴う IME プリエディット
    /// 更新（ADR-0102）。Canvas Mode が文節ごとの変換下線を描ける。`clauses` の
    /// オフセットは `text` 相対。
    pub fn on_composition_update_formatted(
        &mut self,
        target: ElementId,
        text: &str,
        clauses: Vec<crate::element::edit_state::CompositionClause>,
    ) {
        let edited = if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit_with_clauses(text, clauses);
            true
        } else {
            false
        };
        if edited {
            self.bump_a11y_generation();
        }
        self.emit_interaction(Event::CompositionUpdate {
            target_id: target,
            text: text.to_string(),
        });
    }

    pub fn on_composition_end(&mut self, target: ElementId, text: &str) {
        let committed = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
            .map(|edit| {
                edit.finish_composition(text);
                true
            })
            .unwrap_or(false);
        // 確定は preedit を確定値へ畳む＝display_text（a11y value）を変える（#642）。
        if committed {
            self.bump_a11y_generation();
        }
        self.emit_interaction(Event::CompositionEnd {
            target_id: target,
            text: text.to_string(),
        });
        // IME 確定は内容変更（断片ではなく全文の置換）なので、DOM が `compositionend`
        // の直後に `input` を発火するのと同型に、確定後の全文を載せた `TextInput` を
        // 続けて発行する。これが無いと controlled input（value を signal/state にミラー
        // する FW）は確定値を受け取れず、`onInput` が呼ばれないまま draft が空に
        // 留まる（Canvas のみで再現する text-input 追加バグの根本原因）。on_text_input /
        // paste / 複数行 Enter と同じく結合表示テキストを value に載せる（ADR-0069 / #474）。
        if committed {
            let value = self.element_get_text_content(target);
            self.emit_interaction(Event::TextInput {
                target_id: target,
                text: value,
            });
        }
    }

    pub fn on_hover_enter(&mut self, target: ElementId) {
        if self.hover_enter_element(target) {
            self.emit_interaction(Event::HoverEnter { target_id: target });
        }
    }

    pub fn on_hover_leave(&mut self, target: ElementId) {
        if self.hover_leave_element(target) {
            self.emit_interaction(Event::HoverLeave { target_id: target });
        }
    }

    /// プログラム的 focus（mutation バッチ／アクセシビリティ）。
    pub fn on_focus(&mut self, id: ElementId) {
        self.transition_focus(id);
    }

    /// プログラム的 blur（mutation バッチ）。
    pub fn on_blur(&mut self, id: ElementId) {
        self.blur_with_events(id);
    }

    pub fn active_element(&self) -> Option<ElementId> {
        self.interaction.active_element
    }

    /// 文書全体で唯一のテキスト選択（あれば）（ADR-0097）。
    pub fn selection(&self) -> Option<&Selection> {
        self.interaction.selection.as_ref()
    }

    /// active な選択の端点を文書順に正規化した `(start, end)`。`start` はツリーの
    /// pre-order 走査で `end` に先行する（ADR-0097）。同一ブロックの選択はバイト
    /// オフセットで、ブロック跨ぎはブロックの文書順で正規化する。選択が無ければ
    /// `None`。
    pub fn selection_ordered(&self) -> Option<(SelectionPoint, SelectionPoint)> {
        let sel = self.interaction.selection.get()?;
        if sel.anchor.element == sel.focus.element {
            let lo = sel.anchor.offset.min(sel.focus.offset);
            let hi = sel.anchor.offset.max(sel.focus.offset);
            let el = sel.anchor.element;
            return Some((SelectionPoint::new(el, lo), SelectionPoint::new(el, hi)));
        }
        match self.document_order(sel.anchor.element, sel.focus.element) {
            std::cmp::Ordering::Greater => Some((sel.focus, sel.anchor)),
            _ => Some((sel.anchor, sel.focus)),
        }
    }

    /// 文書グローバル選択を `anchor`..`focus` にプログラム設定する（ADR-0097。
    /// ポインタ／キーボードジェスチャによらない選択）。両端点が1つの Selection
    /// Region を共有するとき（最近接の `selectable` 祖先が一致し存在する）だけ
    /// 適用するので、ドラッグと同じ `selectable` 境界を尊重し越境しない。適用したか
    /// を返す。共有の選択経路を通すので、ジェスチャと全く同様にハイライトを再生成
    /// し `selection-change` 通知を発行する。
    pub fn set_selection_range(&mut self, anchor: SelectionPoint, focus: SelectionPoint) -> bool {
        // 両端点が selectable（`user-select: none` でない）で、かつ Selection Region
        // 境界を共有する必要がある。`None` 境界は無制限の文書領域（ADR-0108）なので、
        // 境界を持たない2点はそれを共有し要素跨ぎのプログラム範囲が通る。`contains`
        // （または旧来の `selectable`）境界は依然として閉じ込めるので、境界の両側に
        // ある端点は一致せず範囲は拒否される。
        if self.user_select_excludes(anchor.element) || self.user_select_excludes(focus.element) {
            return false;
        }
        if self.selection_region_of(anchor.element) != self.selection_region_of(focus.element) {
            return false;
        }
        self.set_selection(Some(Selection { anchor, focus }));
        true
    }

    /// 文書グローバル選択をプログラム的にクリアする（ADR-0097）。何も選択されて
    /// いなければ no-op。共有の選択経路を通すので、落とすハイライトを再生成し
    /// `selection-change` 通知を発行する。
    pub fn clear_selection(&mut self) {
        self.set_selection(None);
    }

    /// active な選択が覆う IFC ブロック `block` のバイト範囲を文書順に正規化した
    /// もの（ADR-0097）。同一ブロックの選択ではブロック内範囲。ブロック跨ぎでは、
    /// 先頭ブロックは開始オフセットから末尾まで、末尾ブロックは 0 から終了オフ
    /// セットまで、間のブロックは丸ごと覆われる。選択が `block` に触れない、または
    /// `block` が別の Selection Region に属するとき `None`（選択は `selectable`
    /// 境界を越えない）。
    pub(crate) fn selection_range_in_block(&self, block: ElementId) -> Option<(usize, usize)> {
        // `user-select: none` のブロック（または `none` サブツリー下）は選択を持た
        // ない。ハイライトとコピーテキストは同じこの継ぎ目で覆う範囲を読むので、
        // どちらも同様にスキップされる（ADR-0108）。
        if self.user_select_excludes(block) {
            return None;
        }
        let (start, end) = self.selection_ordered()?;
        if start.element == end.element {
            return (start.element == block).then_some((start.offset, end.offset));
        }
        if self.selection_region_of(block) != self.selection_region_of(start.element) {
            return None;
        }
        let block_len = || self.ifc_text(block).map(|t| t.len()).unwrap_or(0);
        if block == start.element {
            Some((start.offset, block_len()))
        } else if block == end.element {
            Some((0, end.offset))
        } else if self.document_order(start.element, block) == std::cmp::Ordering::Less
            && self.document_order(block, end.element) == std::cmp::Ordering::Less
        {
            Some((0, block_len()))
        } else {
            None
        }
    }

    /// `id` が自身または祖先の `user-select: none` で文書選択から除外されるか
    /// （ADR-0108: `none` はサブツリー全体を除外）。覆う範囲・ハイライト・コピー
    /// テキストの各経路が共有する単一ゲート（すべて `selection_range_in_block`
    /// 経由）なので、`none` 要素は一度に全経路から外れる。
    fn user_select_excludes(&self, id: ElementId) -> bool {
        let mut current = Some(id);
        while let Some(eid) = current {
            let Some(el) = self.elements.get(&eid) else {
                break;
            };
            if el.user_select == crate::element::style::UserSelectValue::None {
                return true;
            }
            current = el.parent;
        }
        false
    }

    /// 2要素を文書順（ツリーの pre-order DFS での位置）で比較する。祖先は子孫に、
    /// 前の兄弟は後の兄弟に先行する。各要素の root-path（根からの子インデックス
    /// 列）を辞書順に比較して実装する。
    fn document_order(&self, a: ElementId, b: ElementId) -> std::cmp::Ordering {
        self.root_path(a).cmp(&self.root_path(b))
    }

    /// 文書根から `id` までの経路を子インデックス列（根相対）で返す。2つの経路を
    /// 辞書順比較すると pre-order になる。接頭辞（祖先）は長い経路（子孫）より前に
    /// 並ぶ。
    fn root_path(&self, id: ElementId) -> Vec<usize> {
        let mut path = Vec::new();
        let mut cur = id;
        while let Some(el) = self.elements.get(&cur) {
            let Some(parent) = el.parent else { break };
            let idx = self
                .elements
                .get(&parent)
                .and_then(|p| p.children.iter().position(|&c| c == cur))
                .unwrap_or(0);
            path.push(idx);
            cur = parent;
        }
        path.reverse();
        path
    }

    /// 選択下のテキストを単一文字列で返す（ADR-0097 / ADR-0108）。選択が覆う
    /// IFC 根ブロックを文書順にたどり（ハイライトが降ろすのと同じ
    /// `selection_range_in_block` 継ぎ目なので、コピーと描画が一致し順序ロジックの
    /// 重複が無い）、各ブロックの覆うバイト範囲を整形済みテキストから切り出し
    /// （インライン子は既に文書順で連結済みなのでスタイル付きランも連結されて
    /// 戻る）、ブロックボックス境界ごとに `\n` を1つ挿入する（ADR-0108: ブラウザ
    /// コピーと同じ形）。選択が無い、または非空の範囲を何も覆わない（畳まれた
    /// キャレットはコピー対象なし）とき `None`。
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.interaction.selection.get()?;
        let mut parts: Vec<String> = Vec::new();
        for block in self.blocks_spanned_by(sel) {
            let Some((start, end)) = self.selection_range_in_block(block) else {
                continue;
            };
            if start == end {
                continue;
            }
            let Some(text) = self.ifc_text(block) else {
                continue;
            };
            parts.push(text[start..end].to_string());
        }
        if parts.is_empty() {
            return None;
        }
        Some(parts.join("\n"))
    }

    /// 現在の操作で選択 chrome（ドラッグハンドルとフローティングツールバー）を
    /// 描くべきか。Touch モダリティのときだけ true（ADR-0104）。Mouse/Pen は細い
    /// キャレットとドラッグ選択のみでデスクトップブラウザに合わせ、Touch はモバイル
    /// ジェスチャ面を出す。[`last_pointer_kind`] から操作ごとに読むので、ハイブリッド
    /// 機（タッチノート、マウス付きタブレット）は現デバイスに追従する。ハイライト
    /// の色付けは意図的にここでゲートしない。全モダリティで描かれる（ADR-0097、
    /// tint=Chromium）。
    ///
    /// [`last_pointer_kind`]: Self::last_pointer_kind
    fn touch_chrome_visible(&self) -> bool {
        self.interaction.last_pointer_kind == PointerKind::Touch
    }

    /// active な選択のためのフローティング選択ツールバー。選択が active でなければ
    /// `None`（ADR-0097）。ツールバーは core 描画の chrome で、読み取り専用
    /// SelectionArea 選択は Copy / Select All を、編集可能 text-input 選択は加えて
    /// Cut / Paste を出す。選択の canvas 空間バウンディングボックス上に浮かび、現在の
    /// chrome スタイルでテーマ付けされる。Touch モダリティでのみ描かれ、Mouse/Pen
    /// 選択は細いキャレットのみ（ADR-0104）。
    pub fn selection_toolbar(&self) -> Option<crate::element::selection_chrome::SelectionToolbar> {
        if !self.touch_chrome_visible() {
            return None;
        }
        let (actions, bounds) = self.active_selection_chrome()?;
        let ct = *self.chrome_tuning();
        let metrics = crate::element::selection_chrome::ToolbarMetrics {
            height: ct.toolbar_height,
            button_pad_x: ct.toolbar_button_pad_x,
            gap: ct.toolbar_gap,
        };
        crate::element::selection_chrome::layout(
            self.selection_chrome_style,
            &actions,
            bounds,
            self.viewport,
            metrics,
            self.interaction.toolbar_overflow_open,
        )
    }

    /// active な読み取り専用選択の両側に並ぶ Material ドラッグハンドルのペア。
    /// 非畳みの SelectionArea 選択が active でなければ `None`（ADR-0097）。ハンドルは
    /// core 描画の chrome で、範囲の各端の直下に涙滴のつまみが下がり、現在の chrome
    /// スタイルでテーマ付けされる。これらはモバイルジェスチャ面（ドラッグでその端点
    /// を調整）なので Touch モダリティでのみ出る。Mouse/Pen 選択は出さない
    /// （ADR-0104）。
    pub fn selection_handles(&self) -> Option<crate::element::selection_chrome::SelectionHandles> {
        if !self.touch_chrome_visible() {
            return None;
        }
        let sel = self.interaction.selection.get()?;
        if sel.is_caret() {
            return None;
        }
        let (start, end) = self.selection_ordered()?;
        let start_caret = self.selection_caret_canvas(start)?;
        let end_caret = self.selection_caret_canvas(end)?;
        Some(crate::element::selection_chrome::layout_handles(
            self.selection_chrome_style,
            start_caret,
            end_caret,
        ))
    }

    /// 読み取り専用選択端点の canvas 空間キャレット端 `(x, baseline_bottom_y)`。
    /// IFC の Parley レイアウトをブロックのキャッシュ原点でオフセットして得る。
    /// 端点のブロックにまだ整形ジオメトリが無いとき `None`。
    fn selection_caret_canvas(&self, point: SelectionPoint) -> Option<(f32, f32)> {
        use parley::{Affinity, Cursor};
        let tl = self.elements.get(&point.element)?.text_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(point.element)?;
        let g = Cursor::from_byte_index(&tl.layout, point.offset, Affinity::Downstream)
            .geometry(&tl.layout, 0.0);
        Some((ex + g.x0 as f32, ey + g.y1 as f32))
    }

    /// 押下 `(x, y)` が選択のドラッグハンドルの一方を掴んだらハンドルドラッグを
    /// 始める（ADR-0097）。掴んだ端が選択の `focus`、反対端が固定 `anchor` になる
    /// ので、既存のドラッグ選択移動経路（[`DragMode::Selection`] →
    /// `update_selection_focus`）がまさにその端点を調整し Selection Region に
    /// クランプする。ハンドルを掴んだ（＝ジェスチャを消費した）かを返す。
    fn begin_handle_drag(&mut self, x: f32, y: f32) -> bool {
        use crate::element::selection_chrome::SelectionHandleEnd;
        let Some(grabbed) = self.selection_handles().and_then(|h| h.handle_at(x, y)) else {
            return false;
        };
        let Some((start, end)) = self.selection_ordered() else {
            return false;
        };
        // 掴んだ端をドラッグし、反対端を anchor として固定する。
        let (anchor, focus) = match grabbed {
            SelectionHandleEnd::Start => (end, start),
            SelectionHandleEnd::End => (start, end),
        };
        self.set_selection(Some(Selection { anchor, focus }));
        self.interaction
            .pointer_gesture
            .begin_drag(DragMode::Selection);
        true
    }

    /// `(x, y)` の pointer-down から Mouse/Pen スクロールバー操作を始める
    /// （ADR-0110）。サム上の押下はドラッグを開始し、トラック余白の押下は Scroll
    /// Offset をクリック方向へ [`SCROLLBAR_PAGE_STEP`] 1つぶんページする。どちらも
    /// ホイールの `apply_wheel_delta` 継ぎ目でコミットするので、同じ Scroll Offset
    /// に収束し（ADR-0046）祖先へ同様に連鎖する（ADR-0084）。Touch は非対話の一時
    /// インジケータを出すため Touch モダリティでは no-op。押下がスクロールバーに
    /// 当たった（＝ジェスチャを消費した）かを返す。
    ///
    /// [`SCROLLBAR_PAGE_STEP`]: crate::element::scene_build::SCROLLBAR_PAGE_STEP
    fn begin_scrollbar_gesture(&mut self, x: f32, y: f32) -> bool {
        use crate::element::scene_build::{ScrollAxis, SCROLLBAR_PAGE_STEP};
        if self.interaction.last_pointer_kind == PointerKind::Touch {
            return false;
        }
        let Some((sv, axis, on_thumb)) = self.scrollbar_hit_at(x, y) else {
            return false;
        };
        if on_thumb {
            // 以降のトラック px ドラッグを offset デルタに対応付け、サムがトラック
            // 空間でポインタを 1:1 で追従するようにする。
            let offset_per_px = if axis.thumb_travel > 0.0 {
                axis.max_offset / axis.thumb_travel
            } else {
                0.0
            };
            let last_pos = match axis.axis {
                ScrollAxis::Vertical => y,
                ScrollAxis::Horizontal => x,
            };
            self.interaction
                .pointer_gesture
                .begin_drag(DragMode::Scrollbar(ScrollbarDrag {
                    scroll_view: sv,
                    axis: axis.axis,
                    last_pos,
                    offset_per_px,
                }));
        } else {
            // トラック余白: クリック方向へページする。サムの遠端を越えれば前方、
            // 近端より手前なら後方へ、どちらも名前付きステップ1つぶん。
            let (tx, ty, tw, th) = axis.thumb;
            let step = match axis.axis {
                ScrollAxis::Vertical => {
                    let forward = y > ty + th;
                    (
                        0.0,
                        if forward {
                            SCROLLBAR_PAGE_STEP
                        } else {
                            -SCROLLBAR_PAGE_STEP
                        },
                    )
                }
                ScrollAxis::Horizontal => {
                    let forward = x > tx + tw;
                    (
                        if forward {
                            SCROLLBAR_PAGE_STEP
                        } else {
                            -SCROLLBAR_PAGE_STEP
                        },
                        0.0,
                    )
                }
            };
            self.apply_wheel_delta(sv, step.0, step.1);
        }
        true
    }

    /// `(x, y)` の下のスクロールバー軸（あれば）を `(scroll_view, geometry,
    /// on_thumb)` で返す。`on_thumb` はサムヒットで true、トラックヒットで false
    /// （ADR-0110）。共有の `scrollbar_axes` ジオメトリを読むのでヒット領域は
    /// オーバーレイが描くものと完全一致する。最も深い（最もネストした）一致
    /// ScrollView が勝つ（そのサムが最後＝最前面に描かれる）。同深度ではサムヒットが
    /// トラックヒットに勝つ。
    fn scrollbar_hit_at(
        &self,
        x: f32,
        y: f32,
    ) -> Option<(
        ElementId,
        crate::element::scene_build::ScrollbarAxisGeometry,
        bool,
    )> {
        let in_rect = |(rx, ry, rw, rh): (f32, f32, f32, f32)| {
            x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
        };
        let mut best: Option<(usize, ElementId, _, bool)> = None;
        for (&id, el) in &self.elements {
            if el.kind != crate::element::kind::ElementKind::ScrollView {
                continue;
            }
            for axis in crate::element::scene_build::scrollbar_axes(self, id) {
                let on_thumb = in_rect(axis.thumb);
                if !on_thumb && !in_rect(axis.track) {
                    continue;
                }
                let depth = self.root_path(id).len();
                let better = match &best {
                    None => true,
                    Some((bd, _, _, bt)) => depth > *bd || (depth == *bd && on_thumb && !*bt),
                };
                if better {
                    best = Some((depth, id, axis, on_thumb));
                }
            }
        }
        best.map(|(_, id, axis, on_thumb)| (id, axis, on_thumb))
    }

    /// 進行中のサムドラッグをポインタ `(x, y)` へ進める（ADR-0110）。前回移動から
    /// のドラッグ軸方向の移動量が Scroll Offset デルタになり、ホイールの継ぎ目
    /// `apply_wheel_delta` でコミットされるので、offset は連続的に動き、この
    /// ScrollView が軸端に達すると未消費の余りが祖先 ScrollView へ連鎖する。
    /// サムドラッグを 1 ステップ進め、更新済み [`ScrollbarDrag`]（次の基準）を返す。
    /// 閾値未満の移動は `None`（状態据え置き）。drag 種別の保持は `Interaction` 側
    /// （`drive_active_drag`）が行うので、ここは幾何コミットと last_pos 更新だけ（#572）。
    fn drag_scrollbar_step(
        &mut self,
        mut drag: ScrollbarDrag,
        x: f32,
        y: f32,
    ) -> Option<ScrollbarDrag> {
        use crate::element::scene_build::ScrollAxis;
        let pos = match drag.axis {
            ScrollAxis::Vertical => y,
            ScrollAxis::Horizontal => x,
        };
        let pointer_delta = pos - drag.last_pos;
        if pointer_delta.abs() < SCROLLBAR_DRAG_MIN_DELTA_PX {
            return None;
        }
        let offset_delta = pointer_delta * drag.offset_per_px;
        match drag.axis {
            ScrollAxis::Vertical => {
                self.apply_wheel_delta(drag.scroll_view, 0.0, offset_delta);
            }
            ScrollAxis::Horizontal => {
                self.apply_wheel_delta(drag.scroll_view, offset_delta, 0.0);
            }
        }
        drag.last_pos = pos;
        Some(drag)
    }

    /// 押下がフローティングツールバーのボタンに当たればそれを実行する。ツールバーが
    /// ジェスチャを消費したかを返す（ADR-0097）。
    fn try_selection_toolbar_tap(&mut self, x: f32, y: f32) -> bool {
        use crate::element::selection_chrome::ToolbarHit;
        let Some(hit) = self.selection_toolbar().and_then(|tb| tb.hit_test(x, y)) else {
            return false;
        };
        match hit {
            // アクション（可視バー or 開いた副メニュー）はそれを実行し、副メニューを畳む。
            ToolbarHit::Action(action) => {
                self.interaction.toolbar_overflow_open = false;
                self.dispatch_toolbar_action(action);
            }
            // ⋮ トグルは副メニューの開閉だけを切り替え、選択は触らない。
            ToolbarHit::Overflow => {
                self.interaction.toolbar_overflow_open = !self.interaction.toolbar_overflow_open;
            }
        }
        true
    }

    /// active な選択に対しツールバーアクションを実行する（ADR-0097）。
    fn dispatch_toolbar_action(&mut self, action: crate::element::selection_chrome::ToolbarAction) {
        use crate::element::selection_chrome::ToolbarAction;
        match action {
            ToolbarAction::Copy => self.copy_active_selection(),
            ToolbarAction::Cut => self.cut_active_selection(),
            ToolbarAction::Paste => self.paste_active_selection(),
            ToolbarAction::SelectAll => self.select_all_active_selection(),
        }
    }

    /// active な選択（読み取り専用 SelectionArea 選択、無ければ編集可能 text-input
    /// の編集選択。単一 active、ADR-0097）下のテキスト。非空が何も選択されて
    /// いなければ `None`。
    fn active_selection_text(&self) -> Option<String> {
        if let Some(text) = self.selected_text() {
            return Some(text);
        }
        let input = self.edit_selection_owner()?;
        let edit = self.elements.get(&input)?.edit.as_ref()?;
        let (start, end) = edit.selection_range()?;
        Some(edit.text_content[start..end].to_string())
    }

    /// active な選択を Platform Adapter のクリップボード経由でコピーする。何も選択
    /// されていないかクリップボード未装着のとき no-op（ADR-0097）。
    fn copy_active_selection(&mut self) {
        if let Some(text) = self.active_selection_text() {
            if let Some(clipboard) = self.clipboard.as_ref() {
                clipboard.write_text(&text);
            }
        }
    }

    /// 編集可能な選択を切り取る。クリップボードへコピーしてから text-input から
    /// 範囲を削除する。読み取り専用 SelectionArea 選択は切り取れないのでそこでは
    /// no-op（ADR-0097）。
    fn cut_active_selection(&mut self) {
        let Some(input) = self.edit_selection_owner() else {
            return;
        };
        let Some(removed) = self
            .elements
            .get_mut(&input)
            .and_then(|el| el.edit.as_mut())
            .and_then(|edit| edit.cut())
        else {
            return;
        };
        if let Some(clipboard) = self.clipboard.as_ref() {
            clipboard.write_text(&removed);
        }
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// 編集可能な選択へクリップボードのテキストを貼り付け、選択を置換する。
    /// Platform Adapter の同期クリップボード読みでテキストを取る。読みが非同期な
    /// アダプタは代わりに結果を `element_paste` で返すのでそこでは no-op。読み取り
    /// 専用選択は貼り付けできない（ADR-0097）。
    fn paste_active_selection(&mut self) {
        let Some(input) = self.edit_selection_owner() else {
            return;
        };
        let Some(text) = self.clipboard.as_ref().and_then(|c| c.read_text()) else {
            return;
        };
        self.element_paste(input, &text);
    }

    /// 特定の text-input へクリップボードのテキストを貼り付ける（キーボードの
    /// Ctrl/Cmd+V 経路、ADR-0103）。`paste_active_selection` と違い focus 中の
    /// フィールドを直接対象にするので、畳まれたキャレット（選択無しの空フィールド）
    /// にも貼り付ける。テキストは Platform Adapter の同期クリップボード読みで取る。
    /// 読みが非同期なアダプタ（Canvas Mode）はここで `None` を返し代わりに
    /// `element_paste` でテキストを返す。
    fn paste_into_text_input(&mut self, target: ElementId) {
        let Some(text) = self.clipboard.as_ref().and_then(|c| c.read_text()) else {
            return;
        };
        self.element_paste(target, &text);
    }

    /// active な選択に対し全選択する。読み取り専用 SelectionArea 選択なら focus
    /// IFC 全体、編集可能なら text-input の内容全体（ADR-0097）。
    fn select_all_active_selection(&mut self) {
        if let Some(sel) = self.interaction.selection.get() {
            self.select_all_in(sel.focus.element);
            return;
        }
        let Some(input) = self.edit_selection_owner() else {
            return;
        };
        if let Some(edit) = self
            .elements
            .get_mut(&input)
            .and_then(|el| el.edit.as_mut())
        {
            let len = edit.text_content.len();
            edit.set_selection(0, len);
        }
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// active な選択をそのツールバーアクション集合と canvas 空間バウンディング
    /// ボックスに解決する。非畳みの読み取り専用 SelectionArea 選択が優先。さもなくば
    /// 非畳みの編集選択を持つ編集可能 text-input（両者は共存しない。単一 active、
    /// ADR-0097）。
    fn active_selection_chrome(
        &self,
    ) -> Option<(
        Vec<crate::element::selection_chrome::ToolbarAction>,
        crate::element::selection_chrome::ToolbarRect,
    )> {
        use crate::element::selection_chrome::ToolbarAction;
        if self
            .interaction
            .selection
            .get()
            .is_some_and(|s| !s.is_caret())
        {
            let bounds = self.read_only_selection_bounds()?;
            return Some((vec![ToolbarAction::Copy, ToolbarAction::SelectAll], bounds));
        }
        let input = self.edit_selection_owner()?;
        let bounds = self.edit_selection_bounds(input)?;
        Some((
            vec![
                ToolbarAction::Cut,
                ToolbarAction::Copy,
                ToolbarAction::Paste,
                ToolbarAction::SelectAll,
            ],
            bounds,
        ))
    }

    /// active（＝focus 中）の非畳み編集選択を持つ text-input（あれば）。選択 chrome
    /// は focus 連動（ADR-0104）。Mouse/Pen 範囲をまだ覚えている非 focus フィールドは
    /// chrome を出さないので、文書全体で active な選択は高々1つ＝focus 中のもの
    /// （単一 active、ADR-0097）。
    fn edit_selection_owner(&self) -> Option<ElementId> {
        let id = self.interaction.focused_element?;
        self.elements
            .get(&id)?
            .edit
            .as_ref()
            .filter(|e| e.selection_range().is_some())
            .map(|_| id)
    }

    /// 読み取り専用選択のハイライトの canvas 空間バウンディングボックス。触れる
    /// ブロック（anchor と focus の IFC）にわたって和を取る。選択にまだ整形
    /// ジオメトリが無いとき `None`。
    fn read_only_selection_bounds(&self) -> Option<crate::element::selection_chrome::ToolbarRect> {
        let (start, end) = self.selection_ordered()?;
        let mut acc: Option<(f32, f32, f32, f32)> = None;
        for block in [start.element, end.element] {
            let Some((s, e)) = self.selection_range_in_block(block) else {
                continue;
            };
            let Some(el) = self.elements.get(&block) else {
                continue;
            };
            let Some(tl) = el.text_layout.as_ref() else {
                continue;
            };
            let Some((ex, ey, _, _)) = self.layout.geometry(block) else {
                continue;
            };
            for (rx, ry, rw, rh) in
                crate::element::scene_build::selection_highlight_rects(&tl.layout, s, e)
            {
                accumulate_rect(&mut acc, ex + rx, ey + ry, rw, rh);
            }
        }
        acc.map(rect_from_bounds)
    }

    /// text-input の編集選択ハイライトの canvas 空間バウンディングボックス。
    fn edit_selection_bounds(
        &self,
        input: ElementId,
    ) -> Option<crate::element::selection_chrome::ToolbarRect> {
        let el = self.elements.get(&input)?;
        let (s, e) = el.edit.as_ref()?.selection_range()?;
        let cl = el.content_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(input)?;
        let taffy_node = self.layout.projection.node_id(input)?;
        let box_layout = self.layout.projection.taffy.layout(taffy_node).ok()?;
        let content_x = ex + box_layout.border.left + box_layout.padding.left;
        let content_y = ey + box_layout.border.top + box_layout.padding.top;
        let mut acc: Option<(f32, f32, f32, f32)> = None;
        for (rx, ry, rw, rh) in
            crate::element::scene_build::selection_highlight_rects(&cl.layout, s, e)
        {
            accumulate_rect(&mut acc, content_x + rx, content_y + ry, rw, rh);
        }
        acc.map(rect_from_bounds)
    }

    /// Selection Region 内の pointer-down から選択を始める。
    ///
    /// - Shift+クリックは（anchor が同じ IFC にあるとき）既存 anchor を保ち focus を
    ///   ヒット点へ移す（範囲拡張）。
    /// - それ以外は同じ箇所付近の押下回数でジェスチャを巡回する。
    ///   1 = キャレット（ドラッグ拡張可）、2 = 単語、3 = 段落。
    ///
    /// `selectable` サブツリー外の押下は選択をクリアし、ドラッグを始めない。
    fn begin_selection_at(&mut self, x: f32, y: f32, modifiers: u32) {
        let Some(point) = self.selection_point_at(x, y) else {
            self.interaction.pointer_gesture.end_drag();
            self.interaction.pointer_gesture.reset_taps();
            if self.interaction.selection.get().is_some() {
                self.set_selection(None);
            }
            return;
        };

        // SelectionArea 選択と text-input 編集選択は排他（単一 active、ADR-0097）。
        self.collapse_edit_selections();

        if modifiers & MOD_SHIFT != 0 && self.extend_focus_to(point) {
            // Shift+クリックは focus を調整。ドラッグを続けられるよう drag に留まる
            // が、マルチクリック周期は進めない。
            self.interaction
                .pointer_gesture
                .begin_drag(DragMode::Selection);
            self.interaction.pointer_gesture.note_single_tap(x, y);
            return;
        }

        match self.interaction.pointer_gesture.classify_tap(x, y) {
            TapPhase::Caret => {
                self.interaction
                    .pointer_gesture
                    .begin_drag(DragMode::Selection);
                self.set_selection(Some(Selection::caret(point)));
            }
            TapPhase::Word => {
                self.interaction.pointer_gesture.end_drag();
                self.select_bounds_at(point, selection::word_bounds);
            }
            TapPhase::Paragraph => {
                self.interaction.pointer_gesture.end_drag();
                self.select_bounds_at(point, selection::line_bounds);
            }
        }
    }

    /// text-input 内の pointer-down からその編集選択を始める（または拡張する）
    /// （ADR-0097）。素の押下はキャレットを落としてドラッグを構え、Shift+クリックは
    /// 既存 anchor から focus を拡張する。同じ箇所付近の連続押下は読み取り専用
    /// SelectionArea 経路（`begin_selection_at`）同様にジェスチャを巡回する。
    /// 1 = キャレット、2 = 単語、3 = 行。単語／行への拡張は Mouse/Pen ジェスチャで、
    /// Touch では押下はキャレットのまま留まるので、長押しの単語選択と競合しない
    /// （ADR-0104）。いずれにせよ読み取り専用 SelectionArea 選択はクリアされる
    /// （単一 active）。押下が編集可能 text-input 内に着地したかを返す。
    fn begin_edit_selection(
        &mut self,
        hit: Option<ElementId>,
        x: f32,
        y: f32,
        modifiers: u32,
    ) -> bool {
        let Some(input) = hit else { return false };
        let is_text_input = self.elements.get(&input).is_some_and(|el| {
            el.kind == crate::element::kind::ElementKind::TextInput && el.edit.is_some()
        });
        if !is_text_input {
            return false;
        }
        let Some(offset) = self.edit_offset_at(input, x, y) else {
            return false;
        };

        // Shift+クリックは新規キャレットでなく既存 anchor から focus を拡張し
        // （範囲拡張）、マルチクリック周期を進めない。読み取り専用
        // `begin_selection_at` の Shift 経路を写す。
        if modifiers & MOD_SHIFT != 0 {
            if let Some(edit) = self
                .elements
                .get_mut(&input)
                .and_then(|el| el.edit.as_mut())
            {
                edit.move_focus(offset);
            }
            self.interaction
                .pointer_gesture
                .begin_drag(DragMode::Edit(input));
            self.interaction.pointer_gesture.note_single_tap(x, y);
            self.finish_edit_selection(input);
            return true;
        }

        // 同じ箇所付近の押下回数でキャレット → 単語 → 行を巡回する。単語と行は
        // Mouse/Pen の拡張で、Touch ではどの押下もキャレットのまま留まる。
        let phase = self.interaction.pointer_gesture.classify_tap(x, y);
        let bounds: Option<fn(&str, usize) -> (usize, usize)> = match (
            phase,
            self.interaction.last_pointer_kind == PointerKind::Touch,
        ) {
            (TapPhase::Word, false) => Some(selection::word_bounds),
            (TapPhase::Paragraph, false) => Some(selection::line_bounds),
            _ => None,
        };
        if let Some(edit) = self
            .elements
            .get_mut(&input)
            .and_then(|el| el.edit.as_mut())
        {
            match bounds {
                Some(bounds) => {
                    let (start, end) = bounds(&edit.text_content, offset);
                    edit.set_selection(start, end);
                }
                None => edit.set_selection(offset, offset),
            }
        }
        // 単語／行選択はドラッグ拡張不可（読み取り専用経路と同等）。キャレットは
        // ユーザが拡張できるようドラッグを構える。
        self.interaction
            .pointer_gesture
            .begin_drag(if bounds.is_none() {
                DragMode::Edit(input)
            } else {
                DragMode::None
            });
        self.finish_edit_selection(input);
        true
    }

    /// [`begin_edit_selection`](Self::begin_edit_selection) の共通末尾。text-input
    /// 選択と SelectionArea 選択は共存しない（単一 active、ADR-0097）ので、文書選択を
    /// クリアしフィールドを再描画する。
    fn finish_edit_selection(&mut self, input: ElementId) {
        self.set_selection(None);
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// 全 text-input の編集選択をキャレットへ畳む。読み取り専用 SelectionArea
    /// 選択が始まるときに呼ばれ、文書全体で active な選択を高々1つに保つ
    /// （ADR-0097）。実際に範囲を持っていたフィールドだけ再描画する。
    fn collapse_edit_selections(&mut self) {
        let collapsed: Vec<ElementId> = self
            .elements
            .iter_mut()
            .filter_map(|(&id, el)| {
                let edit = el.edit.as_mut()?;
                if edit.is_caret() {
                    return None;
                }
                edit.collapse();
                Some(id)
            })
            .collect();
        for id in collapsed {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// 進行中の text-input ドラッグを拡張する。編集選択の focus をポインタ下の
    /// バイトオフセットへ移し、anchor を保つ（ADR-0097）。trait 越しに呼ぶ inherent
    /// 実体（#572）。
    fn extend_edit_drag_to(&mut self, input: ElementId, x: f32, y: f32) {
        let Some(offset) = self.edit_offset_at(input, x, y) else {
            return;
        };
        if let Some(edit) = self
            .elements
            .get_mut(&input)
            .and_then(|el| el.edit.as_mut())
        {
            if edit.cursor_byte_index == offset {
                return;
            }
            edit.move_focus(offset);
        }
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// canvas 点を text-input 内容のバイトオフセットに解決する。要素のコンテンツ
    /// ボックス内（border + padding でインセット。`element_character_bounds` に
    /// 一致）の Parley `content_layout` を使う。フィールド未レイアウトのとき `None`。
    fn edit_offset_at(&self, input: ElementId, x: f32, y: f32) -> Option<usize> {
        let el = self.elements.get(&input)?;
        let cl = el.content_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(input)?;
        let taffy_node = self.layout.projection.node_id(input)?;
        let box_layout = self.layout.projection.taffy.layout(taffy_node).ok()?;
        let content_x = ex + box_layout.border.left + box_layout.padding.left;
        let content_y = ey + box_layout.border.top + box_layout.padding.top;
        Some(byte_index_at_point(cl, x - content_x, y - content_y))
    }

    /// 選択を、IFC の整形済みテキスト内で `bounds` が `point` 周りに計算したバイト
    /// 範囲で置き換える。IFC に整形テキストが無ければキャレットへフォールバックする。
    fn select_bounds_at(
        &mut self,
        point: SelectionPoint,
        bounds: fn(&str, usize) -> (usize, usize),
    ) {
        let Some(text) = self.ifc_text(point.element) else {
            self.set_selection(Some(Selection::caret(point)));
            return;
        };
        let (start, end) = bounds(&text, point.offset);
        self.set_selection(Some(Selection {
            anchor: SelectionPoint::new(point.element, start),
            focus: SelectionPoint::new(point.element, end),
        }));
    }

    /// active な選択の anchor が同じ IFC にあるとき、現在の選択の focus を `point`
    /// へ移し anchor を保つ。適用したかを返す。
    fn extend_focus_to(&mut self, point: SelectionPoint) -> bool {
        let Some(sel) = self.interaction.selection.get() else {
            return false;
        };
        if sel.anchor.element != point.element {
            return false;
        }
        self.set_selection(Some(Selection {
            anchor: sel.anchor,
            focus: point,
        }));
        true
    }

    /// active な選択へキーボード選択ジェスチャを適用し、キーを消費したかを返す。
    ///
    /// - Ctrl/Cmd+A は Selection Region 全体（focus IFC）を選択する。
    /// - Shift+Arrow は focus を1文字、Alt（macOS）または Ctrl（Win/Linux）併用で
    ///   単語単位、移動する。anchor は固定なので、反復押下で範囲が伸縮する。
    fn handle_selection_key(&mut self, key: &str, modifiers: u32) -> bool {
        let Some(sel) = self.interaction.selection.get() else {
            return false;
        };
        if modifiers & MOD_PRIMARY != 0 && key.eq_ignore_ascii_case("a") {
            return self.select_all_in(sel.focus.element);
        }
        if modifiers & MOD_PRIMARY != 0 && key.eq_ignore_ascii_case("c") {
            self.copy_active_selection();
            return true;
        }
        if modifiers & MOD_SHIFT == 0 {
            return false;
        }
        let Some(text) = self.ifc_text(sel.focus.element) else {
            return false;
        };
        let by_word = modifiers & (MOD_ALT | MOD_CTRL) != 0;
        let Some(next) = selection::arrow_step(&text, key, sel.focus.offset, by_word) else {
            return false;
        };
        self.set_selection(Some(Selection {
            anchor: sel.anchor,
            focus: SelectionPoint::new(sel.focus.element, next),
        }));
        true
    }

    /// 単一の編集継ぎ目（ADR-0103）で `target` に [`EditIntent`] を1つ適用し、
    /// 消費したかを返す。Platform Adapter が OS キーストロークを intent に対応付けた
    /// 後に駆動する OS 非依存の入口で、`core` はどのキー由来かを一切調べない。
    ///
    /// 編集可能 text-input で IME 変換が無いときだけ消費する。進行中のプリエディット
    /// は触らず、キャレットキーが変換を壊さない。text-input 選択の変更は読み取り
    /// 専用 SelectionArea 選択をクリアする（単一 active 規則、ADR-0097）。
    pub fn apply_edit_intent(&mut self, target: ElementId, intent: EditIntent) -> bool {
        if !self.can_apply_edit_intent(target, intent) {
            return false;
        }
        // 語彙のうちクリップボード系は Platform Adapter 境界を跨ぐ（ADR-0097）。
        // 以下の分岐は eligibility 検証後だけ実行する。
        if self.element_is_multiline(target) {
            let geometric = match intent {
                EditIntent::Move { direction, .. } | EditIntent::Extend { direction, .. }
                    if matches!(direction, Direction::Up | Direction::Down) =>
                {
                    self.apply_vertical_motion(
                        target,
                        direction,
                        matches!(intent, EditIntent::Extend { .. }),
                    )
                }
                EditIntent::Move {
                    granularity: Granularity::LineBoundary,
                    direction,
                }
                | EditIntent::Extend {
                    granularity: Granularity::LineBoundary,
                    direction,
                } => self.apply_display_line_boundary(
                    target,
                    direction,
                    matches!(intent, EditIntent::Extend { .. }),
                ),
                _ => false,
            };
            if geometric {
                self.set_selection(None);
                return true;
            }
        }
        self.apply_edit_intent_after_eligibility(target, intent)
    }

    /// Platform Adapter が非同期処理（Web paste）を開始する前に、同期 seam と同じ
    /// target/composition/multiline eligibility を問い合わせる。
    pub fn can_apply_edit_intent(&self, target: ElementId, intent: EditIntent) -> bool {
        let Some(el) = self.elements.get(&target) else {
            return false;
        };
        if el.kind != crate::element::kind::ElementKind::TextInput {
            return false;
        }
        let Some(edit) = el.edit.as_ref() else {
            return false;
        };
        if edit.preedit.is_some() {
            return false;
        }
        if intent == EditIntent::InsertLineBreak && !el.multiline {
            return false;
        }
        true
    }

    fn apply_edit_intent_after_eligibility(
        &mut self,
        target: ElementId,
        intent: EditIntent,
    ) -> bool {
        // 語彙のうちクリップボード系は Platform Adapter 境界を跨ぐ（ADR-0097）。
        // システムクリップボードは EditState でなくこの継ぎ目にあるので、ここで
        // ツールバーのクリップボードアクション（既に focus 中 text-input の編集選択に
        // 作用する）を再利用して解決する。純粋状態系（Move / Extend / Delete /
        // SelectAll）は EditState 継ぎ目へ直行する。
        // *複数行*フィールドの垂直移動（↑/↓）と表示行 Home/End は Parley の行
        // ジオメトリを要し、それは純粋 `EditState` でなくここのツリー継ぎ目にある
        // （ADR-0103）。それらを先に解決する。単一行（または未レイアウト）は
        // `EditState::apply` へ落ち、↑/↓ はフィールド端、Home/End はフィールド境界へ
        // 飛ぶ（Chromium `<input>`）。
        match intent {
            EditIntent::InsertLineBreak => {
                let applied = self
                    .elements
                    .get_mut(&target)
                    .and_then(|el| el.edit.as_mut())
                    .is_some_and(|edit| edit.apply(intent));
                if !applied {
                    return false;
                }
                self.engine
                    .mark_visual_dirty(target, VisualInvalidationReach::SelfOnly);
                self.bump_a11y_generation();
                let value = self.element_get_text_content(target);
                self.emit_interaction(Event::TextInput {
                    target_id: target,
                    text: value,
                });
            }
            EditIntent::Copy => self.copy_active_selection(),
            EditIntent::Cut => self.cut_active_selection(),
            EditIntent::Paste => self.paste_into_text_input(target),
            _ => {
                if let Some(edit) = self
                    .elements
                    .get_mut(&target)
                    .and_then(|el| el.edit.as_mut())
                {
                    edit.apply(intent);
                }
                self.engine
                    .mark_visual_dirty(target, VisualInvalidationReach::SelfOnly);
            }
        }
        self.set_selection(None);
        true
    }

    /// `id` が複数行 text-input（`<textarea>` 相当）か。そうなら ↑/↓ は表示行間を
    /// 移動し Home/End は表示行端にスナップする。
    fn element_is_multiline(&self, id: ElementId) -> bool {
        self.elements
            .get(&id)
            .map(|el| el.multiline)
            .unwrap_or(false)
    }

    /// 複数行フィールドでキャレットを表示行1行ぶん上下させ、粘着するゴール列を
    /// 保つ（ADR-0103）。`extend` は anchor を保つ（Shift）。適用したかを返す。
    /// フィールドにまだ整形レイアウトが無いと `false` で、呼び出し側は純粋な単一行
    /// 意味論へフォールバックする。
    fn apply_vertical_motion(
        &mut self,
        target: ElementId,
        direction: Direction,
        extend: bool,
    ) -> bool {
        // 幾何移動解決は `Caret Geometry` seam の裏（`EditState::vertical_motion`）で
        // 純粋計算する（ADR-0122 決定 5）。ここはフィールドの Parley `content_layout`
        // を実 adapter に包み、注入するだけ。整形レイアウトや編集状態が無いと `false`
        // を返し、呼び出し側は単一行意味論へフォールバックする。
        let Some(el) = self.elements.get_mut(&target) else {
            return false;
        };
        let Some(cl) = el.content_layout.as_ref() else {
            return false;
        };
        let Some(edit) = el.edit.as_mut() else {
            return false;
        };
        let geometry = ParleyCaretGeometry::new(&cl.layout);
        let applied = edit.vertical_motion(&geometry, direction, extend);
        if applied {
            self.engine
                .mark_visual_dirty(target, VisualInvalidationReach::SelfOnly);
        }
        applied
    }

    /// 複数行フィールドで現在の*表示*行の先頭／末尾へキャレットを移す（ソフト
    /// ラップ行に対する Home/End、ADR-0103）。`extend` は anchor を保つ
    /// （Shift+Home/End）。適用したかを返す。
    fn apply_display_line_boundary(
        &mut self,
        target: ElementId,
        direction: Direction,
        extend: bool,
    ) -> bool {
        // 表示行 Home/End も `Caret Geometry` seam の裏（`EditState::display_line_boundary`）
        // で計算する（ADR-0122 決定 5）。実 adapter を注入するだけ。
        let Some(el) = self.elements.get_mut(&target) else {
            return false;
        };
        let Some(cl) = el.content_layout.as_ref() else {
            return false;
        };
        let Some(edit) = el.edit.as_mut() else {
            return false;
        };
        let geometry = ParleyCaretGeometry::new(&cl.layout);
        let applied = edit.display_line_boundary(&geometry, direction, extend);
        if applied {
            self.engine
                .mark_visual_dirty(target, VisualInvalidationReach::SelfOnly);
        }
        applied
    }

    /// `ifc` の整形済みテキスト全体を選択する（Ctrl/Cmd+A）。範囲を設定したかを
    /// 返す（要素が整形テキストを持たなければ false）。
    fn select_all_in(&mut self, ifc: ElementId) -> bool {
        let Some(text) = self.ifc_text(ifc) else {
            return false;
        };
        self.set_selection(Some(Selection {
            anchor: SelectionPoint::new(ifc, 0),
            focus: SelectionPoint::new(ifc, text.len()),
        }));
        true
    }

    /// IFC 根の整形済みテキストの連結。バイト境界ジェスチャ向け。
    fn ifc_text(&self, ifc: ElementId) -> Option<std::sync::Arc<str>> {
        self.elements
            .get(&ifc)?
            .text_layout
            .as_ref()
            .map(|tl| tl.text.clone())
    }

    /// canvas 点を選択端点 `(IFC root, byte offset)` に解決する。IFC の Parley
    /// コンテンツレイアウトを使う（ADR-0097）。点が `selectable` サブツリー外か
    /// 整形テキストに当たらないとき `None`。
    fn selection_point_at(&self, x: f32, y: f32) -> Option<SelectionPoint> {
        let hit = self.hit_test(x, y)?;
        if !self.within_selectable(hit) {
            return None;
        }
        let ifc = ifc_root(&self.elements, hit).unwrap_or(hit);
        let tl = self.elements.get(&ifc)?.text_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(ifc)?;
        // drag-to-extend の point→byte は `Caret Geometry` seam（#571）越しに解決する。
        let offset = ParleyCaretGeometry::new(&tl.layout).byte_at_point(x - ex, y - ey);
        Some(SelectionPoint::new(ifc, offset))
    }

    /// `id` で選択を始められるか。実効 `user-select` が `none` でないこと
    /// （ADR-0108）。選択は既定で境界なしなので明示の Selection Region 根は不要。
    /// 種別既定 `user-select: text` の素の段落はドラッグで選択できる。`image` /
    /// `button` は種別既定 `none`、`user-select: none` サブツリーはオプトアウトする
    /// ので、どちらも選択を始めない。点にテキストがあるかは下流の
    /// `selection_point_at` が強制する（非テキストヒットは IFC テキスト無しに解決し
    /// キャレットを生まない）ので、空の `view`（種別既定 `text`）は `none` でなくても
    /// 何も始めない。
    fn within_selectable(&self, id: ElementId) -> bool {
        !self.user_select_excludes(id)
    }

    /// `id` の最近接 Selection Region 根祖先（自身を含む）。選択をそのサブツリーに
    /// 閉じ込める要素。2つのマーカが境界を成す。旧来の `selectable` フラグ
    /// （ADR-0097）と、CSS で記述する封じ込め境界 `user-select: contains`
    /// （ADR-0108）。`id` がどちらの下でもなければ `None`。最近接の祖先が勝つので、
    /// ネストした境界（外側領域内の `contains` ボックス、または `contains` 内の
    /// `contains`）は祖先を隠す。2点が領域を共有するのは最近接根が一致するときだけで、
    /// これが選択の越境を防ぐ。
    fn selection_region_of(&self, id: ElementId) -> Option<ElementId> {
        let mut current = Some(id);
        while let Some(eid) = current {
            let el = self.elements.get(&eid)?;
            if el.selectable || el.user_select == crate::element::style::UserSelectValue::Contains {
                return Some(eid);
            }
            current = el.parent;
        }
        None
    }

    fn set_selection(&mut self, next: Option<Selection>) {
        if self.interaction.selection.get() == next {
            return;
        }
        // 選択が実質的に変われば、別アクション集合/レイアウトの古い ⋮ 副メニューを畳む。
        self.interaction.toolbar_overflow_open = false;
        if let Some(prev) = self.interaction.selection.get() {
            self.mark_selection_dirty(prev);
        }
        // 状態変更は deep module の interface 経由（one-per-document の不変条件を裏で守る）。
        match next {
            Some(selection) => self.interaction.selection.set(selection),
            None => self.interaction.selection.clear(),
        }
        if let Some(now) = self.interaction.selection.get() {
            self.mark_selection_dirty(now);
        }
        // 文書グローバル Selection の実質的変更（設定・移動・クリア。ジェスチャでも
        // プログラム API でも）はホストに一度通知する（ADR-0097）。上の等値ガードに
        // より冗長な設定は何も発しない。意図的にペイロードなし。ホストは DOM の
        // `selectionchange` 同様に `selection()` で新状態をポーリングする。
        self.emit_interaction(Event::SelectionChange);
    }

    /// 選択が覆う各ブロックを再生成し、ハイライトを追従させる。両端点ブロックと、
    /// その間に文書順で並ぶブロックを含むので、ブロック跨ぎ範囲は中間の段落も
    /// 再描画する。
    fn mark_selection_dirty(&mut self, sel: Selection) {
        for block in self.blocks_spanned_by(sel) {
            self.engine
                .mark_visual_dirty(block, VisualInvalidationReach::SelfOnly);
        }
    }

    /// 選択が覆う IFC ブロックを文書順で返す。同一ブロックの選択ならその1ブロック
    /// だけ。さもなくば anchor の Selection Region 内で、先の端点のブロックから後の
    /// ものまでの各 IFC 根。
    fn blocks_spanned_by(&self, sel: Selection) -> Vec<ElementId> {
        if sel.anchor.element == sel.focus.element {
            return vec![sel.anchor.element];
        }
        let region = self.selection_region_of(sel.anchor.element);
        let roots: Vec<ElementId> = self
            .preorder_ifc_roots()
            .filter(|&b| self.selection_region_of(b) == region)
            .collect();
        let ai = roots.iter().position(|&b| b == sel.anchor.element);
        let fi = roots.iter().position(|&b| b == sel.focus.element);
        match (ai, fi) {
            (Some(a), Some(f)) => roots[a.min(f)..=a.max(f)].to_vec(),
            _ => vec![sel.anchor.element, sel.focus.element],
        }
    }

    /// 文書順（文書根からの pre-order DFS）の IFC 根ブロック。
    fn preorder_ifc_roots(&self) -> impl Iterator<Item = ElementId> + '_ {
        let mut out = Vec::new();
        if let Some(root) = self.root {
            let mut stack = vec![root];
            while let Some(id) = stack.pop() {
                if crate::element::inline_text::is_ifc_root(&self.elements, id) {
                    out.push(id);
                }
                if let Some(el) = self.elements.get(&id) {
                    for &child in el.children.iter().rev() {
                        stack.push(child);
                    }
                }
            }
        }
        out.into_iter()
    }

    fn emit_interaction(&mut self, event: Event) {
        if let Some(kind) = event_document_kind(&event) {
            self.dispatch_event(kind, event);
        } else {
            self.push_event(event);
        }
    }

    /// focus 遷移（直前 focus の blur ＋ `Blur` / `Focus` イベント）。`Interaction`
    /// seam へ `Focus` intent として委譲する（ADR-0122）。pointer 経路・accessibility
    /// 経路が同じ seam を共有する。
    pub(crate) fn transition_focus(&mut self, id: ElementId) {
        self.apply_interaction_intent(InteractionIntent::Focus(id));
    }

    /// `id` をイベント付きで blur する（`id` が現在 focus でなければ no-op）。
    /// `Interaction` の同名 seam 操作へ委譲する。
    fn blur_with_events(&mut self, id: ElementId) {
        InteractionSession::new(self).blur_with_events(id);
    }

    /// [`InteractionIntent`] を private session の単一 match に適用する。
    pub fn apply_interaction_intent(&mut self, intent: InteractionIntent) -> InteractionResult {
        InteractionSession::new(self).apply_intent(intent)
    }

    /// 単一 text-input の編集選択をキャレットへ畳み、実際に範囲を持っていたときだけ
    /// 再描画する。文書全体版
    /// [`collapse_edit_selections`](Self::collapse_edit_selections) の blur 時版。
    fn collapse_edit_selection_of(&mut self, id: ElementId) {
        let collapsed = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
            .is_some_and(|edit| {
                if edit.is_caret() {
                    false
                } else {
                    edit.collapse();
                    true
                }
            });
        if collapsed {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    fn apply_pointer_hover(&mut self, deepest_hit: Option<ElementId>) {
        let (entered, left) = self.update_pointer_hover(deepest_hit);
        for id in left {
            self.emit_interaction(Event::HoverLeave { target_id: id });
        }
        for id in entered {
            self.emit_interaction(Event::HoverEnter { target_id: id });
        }
    }
}

/// `acc`（min-x, min-y, max-x, max-y）を矩形 `(x, y, w, h)` を含むよう広げる。
fn accumulate_rect(acc: &mut Option<(f32, f32, f32, f32)>, x: f32, y: f32, w: f32, h: f32) {
    let (x1, y1) = (x + w, y + h);
    *acc = Some(match *acc {
        None => (x, y, x1, y1),
        Some((ax0, ay0, ax1, ay1)) => (ax0.min(x), ay0.min(y), ax1.max(x1), ay1.max(y1)),
    });
}

/// 累積した (min-x, min-y, max-x, max-y) 境界を位置付き矩形に変換する。
fn rect_from_bounds(
    (x0, y0, x1, y1): (f32, f32, f32, f32),
) -> crate::element::selection_chrome::ToolbarRect {
    crate::element::selection_chrome::ToolbarRect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }
}
