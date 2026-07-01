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

/// 編集キーストロークを [`EditIntent`] に対応付ける（ADR-0103）。水平矢印は
/// キャレットを移動（Shift で拡張、Alt/Ctrl で1書記素から単語単位へ拡幅）し、
/// Backspace / Delete は前後1文字を削除する。それ以外のキーは `None` を返し、
/// 呼び出し側は生の `on_key_down` 経路へ落ちる。OS 非依存のコア橋渡しで、
/// 完全な OS キーマップは Platform Adapter が持つ。
fn key_edit_intent(key: &str, modifiers: u32) -> Option<EditIntent> {
    // プライマリ修飾（Win/Linux は Ctrl、macOS は Cmd）でのクリップボード／全選択。
    // これらは矢印と同じ継ぎ目で focus 中の text-input に届く。文書選択経路は
    // 読み取り専用 Selection があるときしか処理しないため、これが無いと focus 中の
    // フィールドは Ctrl/Cmd+A/C/X/V を受け取れない。
    if modifiers & MOD_PRIMARY != 0 {
        if let Some(intent) = clipboard_edit_intent(key) {
            return Some(intent);
        }
    }
    // 前後方向の削除。Alt（macOS Option）または Ctrl（Win/Linux）で書記素から
    // 単語単位へ拡幅する（矢印と同じ「単語単位」修飾）。
    if let Some(direction) = match key {
        "Backspace" => Some(Direction::Backward),
        "Delete" => Some(Direction::Forward),
        _ => None,
    } {
        let granularity = if modifiers & (MOD_ALT | MOD_CTRL) != 0 {
            Granularity::Word
        } else {
            Granularity::Grapheme
        };
        return Some(EditIntent::Delete {
            granularity,
            direction,
        });
    }
    let direction = match key {
        "ArrowLeft" => Direction::Backward,
        "ArrowRight" => Direction::Forward,
        // 垂直移動（素の ↑/↓）。複数行フィールドは表示行間を移動し、単一行は
        // フィールド端へ飛ぶ（下流で解決）。
        "ArrowUp" => Direction::Up,
        "ArrowDown" => Direction::Down,
        _ => return None,
    };
    // Alt/Ctrl は*水平*ステップを単語へ拡幅する。垂直移動には効かず、常に表示行
    // 1行ぶん動く。
    let granularity = if modifiers & (MOD_ALT | MOD_CTRL) != 0
        && matches!(direction, Direction::Backward | Direction::Forward)
    {
        Granularity::Word
    } else {
        Granularity::Grapheme
    };
    Some(if modifiers & MOD_SHIFT != 0 {
        EditIntent::Extend {
            granularity,
            direction,
        }
    } else {
        EditIntent::Move {
            granularity,
            direction,
        }
    })
}

/// プライマリ修飾＋文字をクリップボード／全選択の [`EditIntent`] に対応付ける
/// （ADR-0103）。プライマリ修飾の保持は呼び出し側で確認済みなので、ここは文字
/// だけを見る。それ以外のキーは `None`。
fn clipboard_edit_intent(key: &str) -> Option<EditIntent> {
    if key.eq_ignore_ascii_case("a") {
        Some(EditIntent::SelectAll)
    } else if key.eq_ignore_ascii_case("c") {
        Some(EditIntent::Copy)
    } else if key.eq_ignore_ascii_case("x") {
        Some(EditIntent::Cut)
    } else if key.eq_ignore_ascii_case("v") {
        Some(EditIntent::Paste)
    } else {
        None
    }
}

/// `on_pointer_move` の出力（ADR-0088）。`moved` は 1px dedup で合流されたか
/// レイアウト未準備でスキップされたとき false。`resolved_cursor` はポインタ下の
/// 要素から解決したカーソルで、Platform Adapter がスタイルに触れず OS／ブラウザ
/// カーソルを駆動できる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerMoveResult {
    pub moved: bool,
    pub resolved_cursor: CursorValue,
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
/// 構築するだけで `Interaction::apply_intent` に流す（2 producer = 本物の seam）。
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
    PointerDown { x: f32, y: f32, modifiers: u32 },
    /// pointer-up（#572）。`explicit_target` は active セッションが無いときの HTML
    /// フォールバック。生きた押下があればリリースで Click を確定する（ADR-0082）。
    PointerUp { explicit_target: Option<ElementId> },
    /// canvas `(x, y)` での pointer-move（#572）。hover/cursor 更新と進行中ドラッグの
    /// 駆動を通す。`on_pointer_move` 側で 1px dedup 済み。
    PointerMove { x: f32, y: f32 },
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

/// `Interaction` が tree から借りる狭いビュー（ADR-0122 決定 1）。`Interaction` は
/// 横断的 interaction state を所有し、element 位相・dirty マーク・イベント送出は
/// この trait 越しに行う。fake 実装を与えることで full tree（`commit_frame` /
/// `render`）なしに intent の振る舞いを単体検証できる ＝ interface がそのまま
/// test surface になる。
pub trait InteractionTreeView {
    /// イベントを送出する。document イベントはハンドラ経路へディスパッチし、
    /// それ以外は送出キューに積む（`ElementTree::emit_interaction` と同形）。
    fn emit_event(&mut self, event: Event);
    /// focus 取得の element 側効果：カーソル可視化・visual dirty・`:focus` 擬似
    /// 無効化・カーソル点滅タイマのリセット。focus フィールド自体は `Interaction`
    /// が持つのでここでは触らない。
    fn apply_focus_effects(&mut self, id: ElementId);
    /// blur の element 側効果（`apply_focus_effects` の対）。
    fn apply_blur_effects(&mut self, id: ElementId);
    /// blur 時にアクティブな IME preedit（未確定変換）を確定し、確定後の全文を載せた
    /// `TextInput` イベントを発火する（DOM がフォーカス喪失時に `compositionend`＋
    /// `input` を発火するのと同型・ADR-0069）。preedit が無ければ no-op。これが無いと
    /// 変換中に他所をクリックして blur した controlled input は確定値を受け取れず、
    /// `onInput` が呼ばれないまま draft が空に留まる（Canvas のみで再現する確定バグ）。
    fn commit_preedit_on_blur(&mut self, id: ElementId);
    /// 単一 text-input の編集選択をキャレットへ畳む（Touch blur ライフサイクル、
    /// ADR-0104）。
    fn collapse_edit_selection(&mut self, id: ElementId);
    /// `target`（text-input）へ [`EditIntent`] を適用し、消費したかを返す
    /// （ADR-0103 / ADR-0122 決定 5）。幾何依存操作は `Caret Geometry` seam の裏で
    /// 解決し、クリップボードや選択など tree 側の副作用もここで行う。`Interaction` は
    /// 編集の中身を所有しないので、この借りた seam へ委譲する。
    fn apply_edit(&mut self, target: ElementId, intent: EditIntent) -> bool;
    /// `id` の `:active` 擬似状態の無効化を記録する（active 切替の element 側効果、
    /// ADR-0100）。active フィールド自体は `Interaction` が持つ（#572）。
    fn mark_active_dirty(&mut self, id: ElementId);
    /// 進行中スクロールバーつまみドラッグをポインタ `(x, y)` へ進め、更新された
    /// [`ScrollbarDrag`]（次フレームの基準）を返す（ADR-0110）。閾値未満の移動は
    /// `None`（ドラッグ状態を据え置く）。Scroll Offset コミットは tree 側の幾何。
    fn drag_scrollbar(&mut self, drag: ScrollbarDrag, x: f32, y: f32) -> Option<ScrollbarDrag>;
    /// `input`（text-input）の編集選択ドラッグをポインタ `(x, y)` へ拡張する
    /// （ADR-0097）。point→byte は `Caret Geometry` 幾何で、tree 側に置く。
    fn extend_edit_drag(&mut self, input: ElementId, x: f32, y: f32);
    /// canvas 点 `(x, y)` を選択端点 `(IFC root, byte offset)` に解決する（ADR-0097 /
    /// #574）。point→byte は `Caret Geometry` の `byte_at_point`（#571）。`selectable`
    /// サブツリー外や整形テキストに当たらないとき `None`。
    fn resolve_selection_point(&self, x: f32, y: f32) -> Option<SelectionPoint>;
    /// 2 要素が同じ Selection Region に属するか（選択を境界内に閉じ込める contains
    /// clamp の判定、ADR-0108）。
    fn same_selection_region(&self, a: ElementId, b: ElementId) -> bool;
    /// 文書グローバル選択が `prev` から `new` へ変わったときの tree 側効果：得失した
    /// 各ブロックのハイライト再生成（visual dirty）と一度の `selection-change` 通知。
    fn on_selection_changed(&mut self, prev: Selection, new: Selection);
    /// `target`（text-input）の値を `value` で置換する（AccessKit `SetValue`・#575）。
    /// preedit 確定 → 内容置換 → `TextInput` 発火。`text-input` 以外は no-op。
    fn apply_set_value(&mut self, target: ElementId, value: &str);
    /// 最寄りの祖先 `scroll-view` を `target` が表示に入る最小オフセットへ動かす
    /// （reveal 幾何は tree 側・#575）。既に完全表示か scroll-view 祖先が無ければ no-op。
    fn scroll_to_reveal(&mut self, target: ElementId);
}

/// 横断的 interaction state を所有する deep module（ADR-0122 決定 1）。入口は
/// [`apply_intent`](Self::apply_intent) 単一 seam で、pointer / keyboard /
/// accessibility / edit の意図がここへ合流する。focus に加え、pointer 横断 state
/// （hover / active / press 位置 / `PointerGesture` / modality / pointer-pos / cursor /
/// touch scroll）を所有する（#572）。element 位相・layout 幾何・per-element
/// `EditState`・scroll offset は所有せず、狭い [`InteractionTreeView`] 越しに借りる。
pub struct Interaction {
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

impl Interaction {
    /// 単一 seam：[`InteractionIntent`] を借りたビューに適用する。intent を消費／適用
    /// したかを返す（`Edit` の戻り値が入力の早期消費判定に使われる）。
    pub fn apply_intent(
        &mut self,
        view: &mut dyn InteractionTreeView,
        intent: InteractionIntent,
    ) -> bool {
        match intent {
            InteractionIntent::Focus(id) => {
                self.transition_focus(view, id);
                true
            }
            InteractionIntent::Click { target, x, y } => {
                view.emit_event(Event::Click {
                    target_id: target,
                    x,
                    y,
                });
                true
            }
            InteractionIntent::Edit { target, intent } => view.apply_edit(target, intent),
            InteractionIntent::PointerUp { explicit_target } => {
                self.pointer_up(view, explicit_target);
                true
            }
            InteractionIntent::PointerCancel => {
                self.pointer_cancel(view);
                true
            }
            InteractionIntent::SetValue { target, value } => {
                view.apply_set_value(target, &value);
                true
            }
            InteractionIntent::ScrollToReveal { target } => {
                view.scroll_to_reveal(target);
                true
            }
            // pointer-down / -move の重い hit-test / hover / begin パイプラインは
            // `ElementTree` の dispatch が直接走らせる（`apply_interaction_intent` が
            // mem-take せず分岐するので self.interaction を直読みできる）。ここへは届かない。
            InteractionIntent::PointerDown { .. } | InteractionIntent::PointerMove { .. } => {
                debug_assert!(false, "pointer down/move are dispatched without mem-take");
                false
            }
        }
    }

    /// 現在フォーカスされている要素（あれば）。
    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused_element
    }

    /// `id` をフォーカス要素にし、`Focus` イベントを送出する。直前 focus は
    /// `Blur` イベント付きで blur する。既に focus 済みなら何もしない。
    pub(crate) fn transition_focus(&mut self, view: &mut dyn InteractionTreeView, id: ElementId) {
        if self.focused_element == Some(id) {
            return;
        }
        if let Some(prev) = self.focused_element {
            self.blur_with_events(view, prev);
        }
        self.element_focus(view, id);
        view.emit_event(Event::Focus { target_id: id });
    }

    /// `id` を blur し、モダリティ依存の blur ライフサイクル（ADR-0104）を経て
    /// `Blur` イベントを送出する。Touch は編集選択をキャレットへ畳む。
    pub(crate) fn blur_with_events(&mut self, view: &mut dyn InteractionTreeView, id: ElementId) {
        if self.focused_element != Some(id) {
            return;
        }
        self.element_blur(view, id);
        // DOM パリティ: 変換中（preedit あり）の要素が blur すると、ブラウザは
        // `compositionend`＋`input` を出して確定する。Canvas にはその等価が無いので、
        // ここで preedit を確定し確定値を `TextInput` として発火する。順序も DOM に
        // 倣い、`Blur` の前に出す（ADR-0069）。
        view.commit_preedit_on_blur(id);
        // pointer 種別は `Interaction` 自身が所有するので view 越しではなく自分で読む
        // （#572）。Touch の blur は編集選択をキャレットへ畳む（ADR-0104）。
        if self.last_pointer_kind == PointerKind::Touch {
            view.collapse_edit_selection(id);
        }
        view.emit_event(Event::Blur { target_id: id });
    }

    /// `id` をフォーカス要素にマークする（イベントは出さない focus 切替の原始操作）。
    /// 直前 focus の element 側効果を消し、新 focus の効果を点ける。
    pub(crate) fn element_focus(&mut self, view: &mut dyn InteractionTreeView, id: ElementId) {
        if self.focused_element == Some(id) {
            return;
        }
        if let Some(prev) = self.focused_element {
            view.apply_blur_effects(prev);
        }
        view.apply_focus_effects(id);
        self.focused_element = Some(id);
    }

    /// `id` のフォーカスを外す（`id` が現在フォーカスでなければ no-op）。イベントは
    /// 出さない原始操作。
    pub(crate) fn element_blur(&mut self, view: &mut dyn InteractionTreeView, id: ElementId) {
        if self.focused_element != Some(id) {
            return;
        }
        view.apply_blur_effects(id);
        self.focused_element = None;
    }

    /// アクティブ要素を `next` に切り替え、active 状態が変わる全要素の `:active`
    /// 無効化を view に記録させる（ADR-0100/0089）。dirty マークはフィールド書き込みに
    /// 先行するので、`:active` トランジションは切替前の見た目から始まる。`active_element`
    /// を書くのはこの経路のみ。`None` への切替は保留タップ起点もクリアする（#572）。
    pub(crate) fn set_active(&mut self, view: &mut dyn InteractionTreeView, next: Option<ElementId>) {
        if self.active_element == next {
            return;
        }
        if let Some(prev) = self.active_element {
            view.mark_active_dirty(prev);
        }
        if let Some(now) = next {
            view.mark_active_dirty(now);
        }
        self.active_element = next;
        // 押下が終わる/切り替わると保留中タップの起点は無効になる。
        if next.is_none() {
            self.active_press_pos = None;
        }
    }

    /// pointer-up の状態機械（ADR-0082 / #572）。生きた押下（`active_element`）が
    /// あれば、押下起点座標で `Click` をリリース確定する（スクロールに化けた押下は
    /// `pointer_cancel` で active が消えており Click は出ない）。続けて `ActiveEnd` を
    /// 出し active をクリアする。`explicit_target` は active セッションが無いときの
    /// HTML フォールバック。
    pub(crate) fn pointer_up(
        &mut self,
        view: &mut dyn InteractionTreeView,
        explicit_target: Option<ElementId>,
    ) {
        if let Some(t) = self.active_element {
            let (x, y) = self.active_press_pos.unwrap_or((0.0, 0.0));
            // accessibility の semantic click（#575）と同一の `Click` Event に合流する。
            view.emit_event(Event::Click { target_id: t, x, y });
        }
        let target = self.active_element.or(explicit_target);
        if let Some(t) = target {
            view.emit_event(Event::ActiveEnd { target_id: t });
        }
        self.set_active(view, None);
    }

    /// pointer-cancel の状態機械（#572）。直近 pointer 位置を捨て、進行中ドラッグを
    /// 畳み、生きた押下を `ActiveEnd` 付きで解除する（以降のリリースで Click を発火
    /// させない）。hover 集合のクリアは座標非依存の別経路（呼び出し側が先に行う）。
    pub(crate) fn pointer_cancel(&mut self, view: &mut dyn InteractionTreeView) {
        self.last_pointer_pos = None;
        self.pointer_gesture.end_drag();
        if let Some(t) = self.active_element {
            view.emit_event(Event::ActiveEnd { target_id: t });
        }
        self.set_active(view, None);
    }

    /// 進行中のドラッグを pointer-move `(x, y)` で駆動する（ADR-0066）。種別はジェスチャ
    /// 分類器が単独所有し、三者（スクロールバーつまみ／編集選択／読み取り専用選択）は
    /// 排他なので単一 match で分岐する。幾何（Scroll Offset コミット・point→byte）は
    /// view（tree）側に置き、`Interaction` はドラッグ種別だけを所有する。
    pub(crate) fn drive_active_drag(
        &mut self,
        view: &mut dyn InteractionTreeView,
        x: f32,
        y: f32,
    ) {
        match self.pointer_gesture.drag() {
            DragMode::Scrollbar(drag) => {
                if let Some(updated) = view.drag_scrollbar(drag, x, y) {
                    self.pointer_gesture.begin_drag(DragMode::Scrollbar(updated));
                }
            }
            DragMode::Edit(input) => view.extend_edit_drag(input, x, y),
            DragMode::Selection => {
                // 点→選択端点の解決は view 幾何（`Caret Geometry` byte_at_point・#571）、
                // しかし選択 state は `Interaction` 所有なので拡張は self 上で行う。
                if let Some(point) = view.resolve_selection_point(x, y) {
                    self.extend_selection_focus(view, point);
                }
            }
            DragMode::None => {}
        }
    }

    /// ドラッグ選択の focus を `point` へ拡張する（anchor 固定・ADR-0097 / #574）。
    /// 選択 state は `Interaction` 所有なので self の deep module 上で mutate し（view
    /// 経由だと drive の mem-take placeholder へ書いて失われる）、Selection Region への
    /// contains clamp と dirty／`selection-change` 通知は view へ委ねる。focus が別の
    /// `selectable` 領域へ迷い込んだら据え置く（選択は境界を越えない）。
    pub(crate) fn extend_selection_focus(
        &mut self,
        view: &mut dyn InteractionTreeView,
        point: SelectionPoint,
    ) {
        let Some(prev) = self.selection.get() else {
            return;
        };
        if prev.focus == point {
            return;
        }
        if !view.same_selection_region(point.element, prev.anchor.element) {
            return;
        }
        self.selection.extend_focus(point);
        if let Some(now) = self.selection.get() {
            view.on_selection_changed(prev, now);
        }
    }
}

/// 進行中の Mouse/Pen スクロールバー・サムドラッグ（ADR-0110）。サム上の
/// pointer-down で捕捉し `on_pointer_move` が駆動する。各移動で軸方向の移動量を
/// Scroll Offset デルタに変換し、ホイールと同じ `apply_wheel_delta` 継ぎ目で
/// コミットする（軸端に達した余りは祖先 ScrollView へ連鎖する）。
///
/// 公開 seam [`InteractionTreeView::drag_scrollbar`] の引数／戻り値に現れるため
/// `pub`（trait と同じ可視性。#572）。フィールド型はすべて公開型。
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
    /// canvas 座標でのポインタダウン（ヒットテスト駆動）。
    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.on_pointer_down_with(x, y, 0);
    }

    /// キーボード修飾と物理 [`PointerKind`] を伴うポインタダウン。Platform Adapter
    /// が DOM `PointerEvent.pointerType` を転送し、Core は操作ごとに保持する
    /// (`last_pointer_kind`)。選択／active 挙動は
    /// [`on_pointer_down_with`](Self::on_pointer_down_with) と同一。
    pub fn on_pointer_down_with_kind(
        &mut self,
        x: f32,
        y: f32,
        modifiers: u32,
        kind: PointerKind,
    ) {
        self.interaction.last_pointer_kind = kind;
        self.on_pointer_down_with(x, y, modifiers);
    }

    /// キーボード修飾を伴うポインタダウン（ADR-0097）。Shift は新規選択を始めず
    /// 現在の選択の focus を拡張する。
    pub fn on_pointer_down_with(&mut self, x: f32, y: f32, modifiers: u32) {
        // 新規押下は惰性中のフリック／スプリングバックを中断し、コンテンツを即座に
        // 掴めるようにする（ADR-0082）。慣性は Core が所有するのでここで止める
        // （旧: Platform Adapter 側で個別にクリアしていた）。
        self.interaction.scroll_momentum = None;
        // pointer-down を単一 seam（`InteractionIntent::PointerDown`）に通す（#572）。
        self.apply_interaction_intent(InteractionIntent::PointerDown { x, y, modifiers });
    }

    /// pointer-down の hit-test／消費判定／begin パイプライン本体（#572）。
    /// `apply_interaction_intent` が mem-take せず直接呼ぶので、`self.interaction.
    /// pointer_gesture` 等を直読みでき、挙動は移行前と同一。
    fn dispatch_pointer_down(&mut self, x: f32, y: f32, modifiers: u32) {
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
        self.pointer_down_on_target(Some(target), x, y);
    }

    fn pointer_down_on_target(&mut self, target: Option<ElementId>, x: f32, y: f32) {
        self.interaction.last_input_modality = InputModality::Pointer;
        if let Some(t) = target {
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
        let fallback = self.hit_test(x, y);
        self.pointer_up_with_fallback(fallback);
        self.interaction.pointer_gesture.end_drag();
    }

    /// 物理 [`PointerKind`] を伴うポインタアップ。操作ごとに保持する。リリース挙動
    /// は [`on_pointer_up`](Self::on_pointer_up) と同一。
    pub fn on_pointer_up_with_kind(&mut self, x: f32, y: f32, kind: PointerKind) {
        self.interaction.last_pointer_kind = kind;
        self.on_pointer_up(x, y);
    }

    /// 明示フォールバックターゲットを伴うポインタアップ（HTML Mode）。
    pub fn on_pointer_up_on(&mut self, explicit_target: Option<ElementId>) {
        self.pointer_up_with_fallback(explicit_target);
    }

    fn pointer_up_with_fallback(&mut self, explicit_target: Option<ElementId>) {
        // リリース確定（click-on-release・active 解除）は `Interaction` の状態機械が
        // 所有し、単一 seam（`InteractionIntent::PointerUp`）に通す（ADR-0082 / #572）。
        self.apply_interaction_intent(InteractionIntent::PointerUp { explicit_target });
    }

    /// ポインタキャンセル（タッチ中断／ポインタキャプチャ喪失）。座標非依存で、
    /// hover 集合全体をクリアし（離脱した各要素に `HoverLeave` を発行、保存した
    /// 最終ポインタ位置をリセット。surface-leave の hover クリアと同一）、加えて
    /// active な押下を終える（`active_element.take()` → `ActiveEnd` ＋擬似活性
    /// dirty。pointer-up 経路を写す）。`PointerMove` は捏造しない。
    pub fn on_pointer_cancel(&mut self) {
        // hover 全消去は座標非依存の別経路（surface-leave と同一）。続く active／
        // ドラッグの解除は `Interaction` の状態機械が単一 seam で所有する（#572）。
        self.apply_pointer_hover(None);
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
        self.interaction.last_pointer_kind = kind;
        self.on_pointer_move(x, y)
    }

    /// レイアウトガードと 1px dedup 付きのポインタムーブ。合流時 `moved` は false。
    /// `resolved_cursor` はポインタ下の要素から解決したカーソル（ADR-0088）で、
    /// 合流した移動では不変のまま持ち越す。
    pub fn on_pointer_move(&mut self, x: f32, y: f32) -> PointerMoveResult {
        if !self.has_layout() {
            return PointerMoveResult {
                moved: false,
                resolved_cursor: self.interaction.last_cursor,
            };
        }
        if let Some((lx, ly)) = self.interaction.last_pointer_pos {
            if (x - lx).abs() < POINTER_MOVE_DEDUP_PX && (y - ly).abs() < POINTER_MOVE_DEDUP_PX {
                return PointerMoveResult {
                    moved: false,
                    resolved_cursor: self.interaction.last_cursor,
                };
            }
        }
        self.interaction.last_pointer_pos = Some((x, y));
        // dedup を抜けた実移動だけを単一 seam（`InteractionIntent::PointerMove`）に
        // 通す。hover/cursor 更新と進行中ドラッグの駆動はその裏で行う（#572）。
        self.apply_interaction_intent(InteractionIntent::PointerMove { x, y });
        PointerMoveResult {
            moved: true,
            resolved_cursor: self.interaction.last_cursor,
        }
    }

    /// pointer-move の本体（#572）：`PointerMove` ワイヤイベント送出・hover/cursor の
    /// 更新・進行中ドラッグの駆動。`apply_interaction_intent` が mem-take せず直接呼ぶ
    /// ので `self.interaction.*` を直読み／直書きでき、挙動は移行前と同一。ドラッグ
    /// 駆動の drag-mode dispatch は `Interaction::drive_active_drag` が所有する。
    fn dispatch_pointer_move(&mut self, x: f32, y: f32) {
        self.push_event(Event::PointerMove {
            x,
            y,
            pointer_kind: self.interaction.last_pointer_kind,
        });
        let hit = self.hit_test(x, y);
        self.apply_pointer_hover(hit);
        let resolved_cursor = self.resolve_cursor(hit);
        self.interaction.last_cursor = resolved_cursor;
        self.drive_active_drag(x, y);
    }

    /// 進行中ドラッグの駆動を `Interaction::drive_active_drag` へ委譲する橋（#572）。
    /// `Interaction` を一時取り出し、残りの tree を [`InteractionTreeView`] として
    /// 借りる。drag 種別は `Interaction` 所有なので moved-out 側の実値を読む。
    fn drive_active_drag(&mut self, x: f32, y: f32) {
        let focused = self.interaction.focused_element;
        let mut interaction = std::mem::replace(
            &mut self.interaction,
            Interaction {
                focused_element: focused,
                ..Interaction::default()
            },
        );
        interaction.drive_active_drag(self, x, y);
        self.interaction = interaction;
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
        if let Some((lx, ly)) = self.interaction.last_pointer_pos {
            if (x - lx).abs() < POINTER_MOVE_DEDUP_PX && (y - ly).abs() < POINTER_MOVE_DEDUP_PX {
                return false;
            }
        }
        self.interaction.last_pointer_pos = Some((x, y));
        self.push_event(Event::PointerMove {
            x,
            y,
            pointer_kind: self.interaction.last_pointer_kind,
        });
        true
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
        // focus 中の text-input 内の編集キーは EditIntent に解釈され、単一の編集
        // 継ぎ目で適用される（ADR-0103）。素の矢印はキャレットを移動し（選択は端へ
        // 畳む）、Shift は選択を拡張、Alt/Ctrl は単語単位へ拡幅、Backspace/Delete は
        // 1文字削除する。適用時に消費する（IME 変換中は決して消費せず、削除キーが
        // 変換を壊さない）。
        if let Some(intent) = key_edit_intent(key, modifiers) {
            // 編集は `InteractionIntent::Edit` 封筒として単一 seam を通す（ADR-0122）。
            // pointer/key 経路と accessibility 経路が同じ値型を生産する。
            if self.apply_interaction_intent(InteractionIntent::Edit {
                target: focused,
                intent,
            }) {
                return;
            }
        }
        // Enter は複数行フィールドでのみキャレット位置に改行を挿入する。単一行は
        // テキストに触れず、下記の末尾 KeyDown がアプリの submit シグナルになる。
        // Enter 単体は `apply_key_down` が処理する。
        let multiline = self
            .elements
            .get(&focused)
            .map(|el| el.multiline)
            .unwrap_or(false);
        if key == "Enter" && multiline {
            let inserted = self
                .elements
                .get_mut(&focused)
                .and_then(|el| el.edit.as_mut())
                .map(|edit| edit.apply_key_down(key))
                .unwrap_or(false);
            if inserted {
                // 断片 "\n" ではなく改行挿入後の全文を載せる（上の on_text_input と同型）。
                let value = self.element_get_text_content(focused);
                self.emit_interaction(Event::TextInput {
                    target_id: focused,
                    text: value,
                });
            }
        }
        self.emit_interaction(Event::KeyDown {
            target_id: focused,
            key: key.to_string(),
            modifiers,
        });
    }

    pub fn on_text_input(&mut self, target: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            // キャレット位置に挿入し、選択範囲があれば置換する（ADR-0097）。
            edit.insert(text);
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
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit(text);
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
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit_with_clauses(text, clauses);
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
    pub fn selection_handles(
        &self,
    ) -> Option<crate::element::selection_chrome::SelectionHandles> {
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
        self.interaction.pointer_gesture.begin_drag(DragMode::Selection);
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
            self.interaction.pointer_gesture.begin_drag(DragMode::Scrollbar(ScrollbarDrag {
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
        if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
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
        if self.interaction.selection.get().is_some_and(|s| !s.is_caret()) {
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
            self.interaction.pointer_gesture.begin_drag(DragMode::Selection);
            self.interaction.pointer_gesture.note_single_tap(x, y);
            return;
        }

        match self.interaction.pointer_gesture.classify_tap(x, y) {
            TapPhase::Caret => {
                self.interaction.pointer_gesture.begin_drag(DragMode::Selection);
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
        let is_text_input = self
            .elements
            .get(&input)
            .is_some_and(|el| {
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
            if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
                edit.move_focus(offset);
            }
            self.interaction.pointer_gesture.begin_drag(DragMode::Edit(input));
            self.interaction.pointer_gesture.note_single_tap(x, y);
            self.finish_edit_selection(input);
            return true;
        }

        // 同じ箇所付近の押下回数でキャレット → 単語 → 行を巡回する。単語と行は
        // Mouse/Pen の拡張で、Touch ではどの押下もキャレットのまま留まる。
        let phase = self.interaction.pointer_gesture.classify_tap(x, y);
        let bounds: Option<fn(&str, usize) -> (usize, usize)> =
            match (phase, self.interaction.last_pointer_kind == PointerKind::Touch) {
                (TapPhase::Word, false) => Some(selection::word_bounds),
                (TapPhase::Paragraph, false) => Some(selection::line_bounds),
                _ => None,
            };
        if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
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
        self.interaction.pointer_gesture.begin_drag(if bounds.is_none() {
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
        if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
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
    fn select_bounds_at(&mut self, point: SelectionPoint, bounds: fn(&str, usize) -> (usize, usize)) {
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
                EditIntent::Move { granularity: Granularity::LineBoundary, direction }
                | EditIntent::Extend { granularity: Granularity::LineBoundary, direction } => self
                    .apply_display_line_boundary(
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
        match intent {
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
        self.elements.get(&id).map(|el| el.multiline).unwrap_or(false)
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
        let mut interaction = std::mem::take(&mut self.interaction);
        interaction.blur_with_events(self, id);
        self.interaction = interaction;
    }

    /// [`InteractionIntent`] を `Interaction` deep module に適用する単一の橋
    /// （ADR-0122）。`Interaction` を一時的に取り出すことで、残りの `ElementTree`
    /// を [`InteractionTreeView`] として排他借用できる（単一書き手・aliasing なし）。
    pub(crate) fn apply_interaction_intent(&mut self, intent: InteractionIntent) -> bool {
        // pointer-down / -move は hit-test・hover・begin パイプラインが
        // `self.interaction.pointer_gesture` 等を直読み／直書きするので、mem-take した
        // placeholder では壊れる。これらは tree dispatch を直接走らせて seam を通す
        // （intent 封筒が 2-producer の seam 値。#572）。
        match &intent {
            InteractionIntent::PointerDown { x, y, modifiers } => {
                let (x, y, modifiers) = (*x, *y, *modifiers);
                self.dispatch_pointer_down(x, y, modifiers);
                return true;
            }
            InteractionIntent::PointerMove { x, y } => {
                let (x, y) = (*x, *y);
                self.dispatch_pointer_move(x, y);
                return true;
            }
            _ => {}
        }
        // `Interaction` を一時取り出して残りの tree を排他借用する（単一書き手）。
        // ただし seam の裏で走る tree 側ロジック（`Edit` arm 越しの `edit_selection_owner`
        // 等が現在 focus 中 text-input を解決する）が focus を読むため、placeholder にも
        // 引き継ぎ、apply 中も正しい focus を観測できるようにする。
        let focused = self.interaction.focused_element;
        let mut interaction = std::mem::replace(
            &mut self.interaction,
            Interaction {
                focused_element: focused,
                ..Interaction::default()
            },
        );
        let consumed = interaction.apply_intent(self, intent);
        self.interaction = interaction;
        consumed
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

/// `Interaction` が借りる狭いビューを `ElementTree` の既存 seam 上に実装する
/// （ADR-0122 決定 1）。focus フィールドは `Interaction` 側にあり、ここは element
/// 位相・dirty・イベント送出といった tree 側効果だけを提供する。
impl InteractionTreeView for ElementTree {
    fn emit_event(&mut self, event: Event) {
        self.emit_interaction(event);
    }

    fn apply_focus_effects(&mut self, id: ElementId) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.cursor_visible = true;
        }
        self.engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        self.mark_pseudo_activation_dirty(id, crate::element::pseudo_state::PseudoState::Focus);
        self.layout.last_cursor_toggle_ms = None;
    }

    fn apply_blur_effects(&mut self, id: ElementId) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.cursor_visible = false;
        }
        self.engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        self.mark_pseudo_activation_dirty(id, crate::element::pseudo_state::PseudoState::Focus);
        self.layout.last_cursor_toggle_ms = None;
    }

    fn collapse_edit_selection(&mut self, id: ElementId) {
        self.collapse_edit_selection_of(id);
    }

    fn commit_preedit_on_blur(&mut self, id: ElementId) {
        let committed = self
            .elements
            .get_mut(&id)
            .filter(|el| el.kind == crate::element::kind::ElementKind::TextInput)
            .and_then(|el| el.edit.as_mut())
            .is_some_and(|edit| {
                if edit.preedit.is_some() {
                    edit.commit_preedit();
                    true
                } else {
                    false
                }
            });
        if committed {
            // on_text_input / paste / compositionend と同じく結合表示テキスト全文を
            // value に載せる（ADR-0069 / #474）。確定後は preedit が無いので
            // display == text_content。
            let value = self.element_get_text_content(id);
            self.emit_interaction(Event::TextInput {
                target_id: id,
                text: value,
            });
        }
    }

    fn apply_edit(&mut self, target: ElementId, intent: EditIntent) -> bool {
        // tree 側の編集 seam（クリップボード・選択・`Caret Geometry` 注入を内包）へ
        // 委譲する。`EditIntent` の edit 専用シーム意味は不変（ADR-0103）。
        self.apply_edit_intent(target, intent)
    }

    fn mark_active_dirty(&mut self, id: ElementId) {
        self.mark_pseudo_activation_dirty(id, crate::element::pseudo_state::PseudoState::Active);
    }

    fn drag_scrollbar(&mut self, drag: ScrollbarDrag, x: f32, y: f32) -> Option<ScrollbarDrag> {
        self.drag_scrollbar_step(drag, x, y)
    }

    fn extend_edit_drag(&mut self, input: ElementId, x: f32, y: f32) {
        self.extend_edit_drag_to(input, x, y);
    }

    fn resolve_selection_point(&self, x: f32, y: f32) -> Option<SelectionPoint> {
        self.selection_point_at(x, y)
    }

    fn same_selection_region(&self, a: ElementId, b: ElementId) -> bool {
        self.selection_region_of(a) == self.selection_region_of(b)
    }

    fn on_selection_changed(&mut self, prev: Selection, new: Selection) {
        self.mark_selection_dirty(prev);
        self.mark_selection_dirty(new);
        // 文書グローバル Selection の実質的変更を一度通知する（ADR-0097）。
        self.emit_interaction(Event::SelectionChange);
    }

    fn apply_set_value(&mut self, target: ElementId, value: &str) {
        self.apply_semantic_set_value(target, value);
    }

    fn scroll_to_reveal(&mut self, target: ElementId) {
        self.scroll_into_view(target);
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

#[cfg(test)]
mod seam_tests {
    use super::*;

    fn id(raw: u64) -> ElementId {
        ElementId::from_u64(raw)
    }

    /// 全 tree 効果を記録する fake `InteractionTreeView`。full tree / `commit_frame` /
    /// `render` を立てずに `apply_intent` の振る舞い（状態遷移と発火 Event）を観測する。
    struct FakeView {
        events: Vec<Event>,
        focus_effects: Vec<ElementId>,
        blur_effects: Vec<ElementId>,
        collapsed: Vec<ElementId>,
        /// `commit_preedit_on_blur` が呼ばれた要素の記録（blur 時 preedit 確定の観測）。
        preedit_commits: Vec<ElementId>,
        /// `Edit` arm が seam を通って届いた `(target, intent)` の記録。
        edits: Vec<(ElementId, EditIntent)>,
        /// `apply_edit` が返す「消費した」フラグ（テストごとに差し替える）。
        edit_consumes: bool,
        /// `:active` 無効化をマークされた要素（active 切替の観測）。
        active_dirty: Vec<ElementId>,
        /// `drive_active_drag` が呼んだドライブ種別の記録（drag-mode dispatch の観測）。
        scrollbar_drives: Vec<(f32, f32)>,
        edit_drives: Vec<(ElementId, f32, f32)>,
        /// `resolve_selection_point` が返す点（テストごとに差し替える）。
        resolve_point: Option<SelectionPoint>,
        /// `same_selection_region` が返す値（既定 true）。
        same_region: bool,
        /// `on_selection_changed` で観測した `(prev, new)`。
        selection_changes: Vec<(Selection, Selection)>,
        /// `apply_set_value` で届いた `(target, value)`（accessibility SetValue の観測）。
        set_values: Vec<(ElementId, String)>,
        /// `scroll_to_reveal` で届いた target（accessibility ScrollToReveal の観測）。
        reveals: Vec<ElementId>,
    }

    impl Default for FakeView {
        fn default() -> Self {
            Self {
                events: Vec::new(),
                focus_effects: Vec::new(),
                blur_effects: Vec::new(),
                collapsed: Vec::new(),
                preedit_commits: Vec::new(),
                edits: Vec::new(),
                edit_consumes: true,
                active_dirty: Vec::new(),
                scrollbar_drives: Vec::new(),
                edit_drives: Vec::new(),
                resolve_point: None,
                same_region: true,
                selection_changes: Vec::new(),
                set_values: Vec::new(),
                reveals: Vec::new(),
            }
        }
    }

    impl InteractionTreeView for FakeView {
        fn emit_event(&mut self, event: Event) {
            self.events.push(event);
        }
        fn apply_focus_effects(&mut self, id: ElementId) {
            self.focus_effects.push(id);
        }
        fn apply_blur_effects(&mut self, id: ElementId) {
            self.blur_effects.push(id);
        }
        fn collapse_edit_selection(&mut self, id: ElementId) {
            self.collapsed.push(id);
        }
        fn commit_preedit_on_blur(&mut self, id: ElementId) {
            self.preedit_commits.push(id);
        }
        fn apply_edit(&mut self, target: ElementId, intent: EditIntent) -> bool {
            self.edits.push((target, intent));
            self.edit_consumes
        }
        fn mark_active_dirty(&mut self, id: ElementId) {
            self.active_dirty.push(id);
        }
        fn drag_scrollbar(&mut self, drag: ScrollbarDrag, x: f32, y: f32) -> Option<ScrollbarDrag> {
            self.scrollbar_drives.push((x, y));
            // 更新済み drag を返し、`drive_active_drag` の gesture 再設定経路も回す。
            Some(drag)
        }
        fn extend_edit_drag(&mut self, input: ElementId, x: f32, y: f32) {
            self.edit_drives.push((input, x, y));
        }
        fn resolve_selection_point(&self, _x: f32, _y: f32) -> Option<SelectionPoint> {
            self.resolve_point
        }
        fn same_selection_region(&self, _a: ElementId, _b: ElementId) -> bool {
            self.same_region
        }
        fn on_selection_changed(&mut self, prev: Selection, new: Selection) {
            self.selection_changes.push((prev, new));
        }
        fn apply_set_value(&mut self, target: ElementId, value: &str) {
            self.set_values.push((target, value.to_string()));
        }
        fn scroll_to_reveal(&mut self, target: ElementId) {
            self.reveals.push(target);
        }
    }

    #[test]
    fn focus_intent_on_unfocused_focuses_and_emits_focus() {
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let a = id(1);

        interaction.apply_intent(&mut view, InteractionIntent::Focus(a));

        assert_eq!(interaction.focused_element(), Some(a));
        assert_eq!(view.focus_effects, vec![a]);
        assert!(
            matches!(view.events.as_slice(), [Event::Focus { target_id }] if *target_id == a),
            "expected a single Focus event for the target, got {:?}",
            view.events,
        );
    }

    #[test]
    fn focus_intent_over_existing_blurs_prev_then_focuses_new() {
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let (a, b) = (id(1), id(2));
        interaction.apply_intent(&mut view, InteractionIntent::Focus(a));
        view.events.clear();
        view.focus_effects.clear();
        view.blur_effects.clear();

        interaction.apply_intent(&mut view, InteractionIntent::Focus(b));

        assert_eq!(interaction.focused_element(), Some(b));
        assert_eq!(view.blur_effects, vec![a]);
        assert_eq!(view.focus_effects, vec![b]);
        assert!(
            matches!(
                view.events.as_slice(),
                [Event::Blur { target_id: blurred }, Event::Focus { target_id: focused }]
                    if *blurred == a && *focused == b
            ),
            "expected Blur(prev) then Focus(new), got {:?}",
            view.events,
        );
    }

    #[test]
    fn focus_intent_on_already_focused_is_noop() {
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let a = id(1);
        interaction.apply_intent(&mut view, InteractionIntent::Focus(a));
        view.events.clear();
        view.focus_effects.clear();
        view.blur_effects.clear();

        interaction.apply_intent(&mut view, InteractionIntent::Focus(a));

        assert_eq!(interaction.focused_element(), Some(a));
        assert!(view.events.is_empty(), "no events on re-focus, got {:?}", view.events);
        assert!(view.focus_effects.is_empty());
        assert!(view.blur_effects.is_empty());
    }

    #[test]
    fn touch_blur_collapses_edit_selection_before_blur_event() {
        // pointer 種別は `Interaction` が所有する（#572）ので、それを Touch にする。
        let mut interaction = Interaction {
            last_pointer_kind: PointerKind::Touch,
            ..Interaction::default()
        };
        let mut view = FakeView::default();
        let (a, b) = (id(1), id(2));
        interaction.apply_intent(&mut view, InteractionIntent::Focus(a));
        view.events.clear();

        interaction.apply_intent(&mut view, InteractionIntent::Focus(b));

        assert_eq!(view.collapsed, vec![a], "touch blur collapses the blurred input");
    }

    #[test]
    fn mouse_blur_does_not_collapse_edit_selection() {
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let (a, b) = (id(1), id(2));
        interaction.apply_intent(&mut view, InteractionIntent::Focus(a));

        interaction.apply_intent(&mut view, InteractionIntent::Focus(b));

        assert!(view.collapsed.is_empty(), "mouse blur keeps the range in EditState");
    }

    #[test]
    fn click_intent_emits_click_at_point() {
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let target = id(7);

        interaction.apply_intent(
            &mut view,
            InteractionIntent::Click {
                target,
                x: 12.0,
                y: 34.0,
            },
        );

        assert!(
            matches!(
                view.events.as_slice(),
                [Event::Click { target_id, x, y }]
                    if *target_id == target && *x == 12.0 && *y == 34.0
            ),
            "expected a single Click at (12,34), got {:?}",
            view.events,
        );
    }

    #[test]
    fn edit_intent_routes_through_seam_to_apply_edit() {
        // `Edit` arm は `apply_intent` 経由で view の `apply_edit` へ届き、その戻り値
        // （消費したか）をそのまま返す（ADR-0122 決定 5）。
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let target = id(3);
        let intent = EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Up,
        };

        let consumed = interaction.apply_intent(&mut view, InteractionIntent::Edit { target, intent });

        assert!(consumed, "apply_edit が true を返せば intent も消費扱い");
        assert_eq!(view.edits, vec![(target, intent)], "EditIntent が seam を通って届く");
    }

    #[test]
    fn edit_intent_reports_unconsumed_when_apply_edit_declines() {
        // text-input でない等で `apply_edit` が false なら、入力は消費されない
        // （`on_key_down` が生の KeyDown 経路へ落ちられる）。
        let mut interaction = Interaction::default();
        let mut view = FakeView {
            edit_consumes: false,
            ..FakeView::default()
        };
        let intent = EditIntent::Delete {
            granularity: Granularity::Grapheme,
            direction: Direction::Backward,
        };

        let consumed =
            interaction.apply_intent(&mut view, InteractionIntent::Edit { target: id(9), intent });

        assert!(!consumed, "apply_edit が false なら未消費");
    }

    // --- #572: pointer 状態機械（click-on-release / pointer-cancel / drag-mode）---

    #[test]
    fn pointer_up_fires_click_on_release_at_press_point() {
        // 生きた押下があれば、リリースで押下起点座標の Click → ActiveEnd を出し、
        // active をクリアして `:active` 無効化を記録する（ADR-0082 / #572）。
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let t = id(5);
        interaction.active_element = Some(t);
        interaction.active_press_pos = Some((3.0, 4.0));

        interaction.apply_intent(&mut view, InteractionIntent::PointerUp { explicit_target: None });

        assert!(
            matches!(
                view.events.as_slice(),
                [Event::Click { target_id, x, y }, Event::ActiveEnd { target_id: ae }]
                    if *target_id == t && *x == 3.0 && *y == 4.0 && *ae == t
            ),
            "expected Click(press) then ActiveEnd, got {:?}",
            view.events,
        );
        assert_eq!(interaction.active_element, None, "release clears active");
        assert_eq!(view.active_dirty, vec![t], ":active invalidation recorded");
    }

    #[test]
    fn pointer_up_without_a_live_press_fires_no_click() {
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();

        interaction.apply_intent(&mut view, InteractionIntent::PointerUp { explicit_target: None });

        assert!(view.events.is_empty(), "no live press → no click, got {:?}", view.events);
    }

    #[test]
    fn pointer_up_uses_explicit_target_for_active_end_without_active() {
        // active セッションが無ければ Click は出ないが、明示ターゲットへ ActiveEnd は
        // 出る（HTML フォールバック）。
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let t = id(9);

        interaction.apply_intent(&mut view, InteractionIntent::PointerUp { explicit_target: Some(t) });

        assert!(
            matches!(view.events.as_slice(), [Event::ActiveEnd { target_id }] if *target_id == t),
            "expected only ActiveEnd for the explicit target, got {:?}",
            view.events,
        );
    }

    #[test]
    fn pointer_cancel_clears_active_so_release_fires_no_click() {
        // キャンセルは生きた押下を解除する（スクロール乗っ取りで化けた押下が、後続の
        // リリースで Click を発火しない・ADR-0082）。
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let t = id(2);
        interaction.active_element = Some(t);
        interaction.active_press_pos = Some((1.0, 1.0));

        interaction.apply_intent(&mut view, InteractionIntent::PointerCancel);

        assert_eq!(interaction.active_element, None, "cancel clears the live press");
        assert!(
            matches!(view.events.as_slice(), [Event::ActiveEnd { target_id }] if *target_id == t),
            "cancel emits ActiveEnd, got {:?}",
            view.events,
        );

        view.events.clear();
        interaction.apply_intent(&mut view, InteractionIntent::PointerUp { explicit_target: None });
        assert!(
            view.events.is_empty(),
            "a cancelled press fires no click on release, got {:?}",
            view.events,
        );
    }

    #[test]
    fn drive_active_drag_dispatches_to_the_classified_drag_mode() {
        // 進行中ドラッグ種別ごとに、対応する view ドライブだけが呼ばれる（三者排他、
        // ADR-0066）。
        let input = id(4);
        let mut interaction = Interaction::default();
        interaction.pointer_gesture.begin_drag(DragMode::Edit(input));
        let mut view = FakeView::default();
        interaction.drive_active_drag(&mut view, 10.0, 20.0);
        assert_eq!(view.edit_drives, vec![(input, 10.0, 20.0)]);
        assert!(view.scrollbar_drives.is_empty());
    }

    #[test]
    fn drive_active_drag_extends_the_document_selection_across_elements() {
        // 読み取り専用 SelectionArea ドラッグは、view が解決した点（`byte_at_point` ＝
        // `Caret Geometry` #571）へ `Interaction` 所有の選択を要素をまたいで拡張する
        // （#574）。anchor は固定、focus がドラッグ点へ。
        let mut interaction = Interaction::default();
        interaction.pointer_gesture.begin_drag(DragMode::Selection);
        interaction
            .selection
            .set(Selection::caret(SelectionPoint::new(id(1), 2)));
        let drag_point = SelectionPoint::new(id(2), 5);
        let mut view = FakeView {
            resolve_point: Some(drag_point),
            ..FakeView::default()
        };

        interaction.drive_active_drag(&mut view, 7.0, 8.0);

        let sel = interaction.selection.get().unwrap();
        assert_eq!(sel.anchor, SelectionPoint::new(id(1), 2), "anchor は固定");
        assert_eq!(sel.focus, drag_point, "focus が要素をまたいでドラッグ点へ");
        assert_eq!(view.selection_changes.len(), 1, "選択変更が一度通知される");
        assert!(view.edit_drives.is_empty() && view.scrollbar_drives.is_empty());
    }

    #[test]
    fn drag_select_does_not_cross_a_selection_region_boundary() {
        // focus が別 Selection Region へ迷い込んだら据え置く（選択は境界を越えない）。
        let mut interaction = Interaction::default();
        interaction.pointer_gesture.begin_drag(DragMode::Selection);
        interaction
            .selection
            .set(Selection::caret(SelectionPoint::new(id(1), 2)));
        let mut view = FakeView {
            resolve_point: Some(SelectionPoint::new(id(2), 5)),
            same_region: false, // 別領域
            ..FakeView::default()
        };

        interaction.drive_active_drag(&mut view, 7.0, 8.0);

        assert_eq!(
            interaction.selection.get().unwrap().focus,
            SelectionPoint::new(id(1), 2),
            "境界外の点では focus を据え置く",
        );
        assert!(view.selection_changes.is_empty(), "変更通知も出ない");
    }

    #[test]
    fn drive_active_drag_with_no_drag_is_a_noop() {
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();

        interaction.drive_active_drag(&mut view, 1.0, 1.0);

        assert!(
            view.edit_drives.is_empty()
                && view.selection_changes.is_empty()
                && view.scrollbar_drives.is_empty(),
            "DragMode::None drives nothing",
        );
    }

    #[test]
    fn drive_active_drag_advances_scrollbar_and_keeps_the_updated_drag() {
        // スクロールバーつまみドラッグは view へ駆動を委ね、返った更新 drag を次の
        // 基準として gesture に保持する（ADR-0110 / #572）。
        use crate::element::scene_build::ScrollAxis;
        let sv = id(11);
        let drag = ScrollbarDrag {
            scroll_view: sv,
            axis: ScrollAxis::Vertical,
            last_pos: 0.0,
            offset_per_px: 1.0,
        };
        let mut interaction = Interaction::default();
        interaction.pointer_gesture.begin_drag(DragMode::Scrollbar(drag));
        let mut view = FakeView::default();

        interaction.drive_active_drag(&mut view, 0.0, 25.0);

        assert_eq!(view.scrollbar_drives, vec![(0.0, 25.0)], "scrollbar drag driven via view");
        assert!(
            matches!(interaction.pointer_gesture.drag(), DragMode::Scrollbar(d) if d.scroll_view == sv),
            "the updated scrollbar drag is retained for the next move",
        );
    }

    // --- #575: accessibility inbound が同一 seam を共有する ---

    #[test]
    fn set_value_intent_routes_through_seam_to_apply_set_value() {
        // AccessKit `SetValue` は `InteractionIntent::SetValue` として apply_intent を
        // 通り、view の `apply_set_value`（編集経路と同一の実体）へ届く（#575）。
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let target = id(8);

        let consumed = interaction.apply_intent(
            &mut view,
            InteractionIntent::SetValue {
                target,
                value: "hello".to_string(),
            },
        );

        assert!(consumed);
        assert_eq!(view.set_values, vec![(target, "hello".to_string())]);
    }

    #[test]
    fn scroll_to_reveal_intent_routes_through_seam() {
        // AccessKit `ScrollIntoView` は `ScrollToReveal` として apply_intent を通り、
        // reveal 幾何を持つ view の `scroll_to_reveal` へ届く（adapter は intent のみ・#575）。
        let mut interaction = Interaction::default();
        let mut view = FakeView::default();
        let target = id(6);

        let consumed =
            interaction.apply_intent(&mut view, InteractionIntent::ScrollToReveal { target });

        assert!(consumed);
        assert_eq!(view.reveals, vec![target]);
    }
}
