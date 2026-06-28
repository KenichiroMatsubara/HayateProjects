use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use linebender_resource_handle::Blob;
use crate::color::Color;
use crate::element::document_runtime::{self, DocumentRuntime, EventDelivery, ListenerId};
use crate::element::edit_state::EditState;
use crate::element::engine::ElementEngine;
use crate::element::effective_visual::{self, child_inherited_context};
use crate::element::viewport_resize;
use crate::element::ime_bridge::{CharacterBounds, ImeBridge, ImePresentation};
use crate::element::event_spec::DocumentEventKind;

pub use crate::element::event_spec::Event;
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::inline_text::{self, ifc_root};
use crate::element::layout_pass::LayoutPass;
use crate::element::taffy_projection::{TaffyProjection, TraversalStep};
use crate::element::pseudo_state::{
    self, diff_hover_sets, hover_set_for_hit, InteractionSnapshot, PseudoState, PseudoStyles,
};
use crate::element::scene_build;
use crate::element::scene_lowering::{collect_lowering_dirty, SceneLowering};
use crate::element::style::{
    BorderStyleValue, CursorValue, FontStyleValue, OverflowValue, Shadow, StyleProp, StylePropKind,
    TextDecorationValue, TextOverflowValue, TransitionTimingValue, ViewportCondition,
};
use crate::element::text;
use crate::element::visual_invalidation::{
    self, Change, DirtyKind, DirtySink, ElementContext, VisualInvalidationReach,
};
use crate::node::SceneGraph;
use crate::render::RenderImage;

#[derive(Clone, Debug)]
pub struct Visual {
    pub background_color: Option<Color>,
    pub opacity: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub border_color: Option<Color>,
    pub border_style: BorderStyleValue,
    /// box-shadow レイヤーを描画順に保持（ADR-0095）。空なら影なし。CSS の
    /// paint order に合わせ最前面レイヤーが先頭。
    pub box_shadow: Vec<Shadow>,
    /// 子要素のオーバーフロー処理。`Hidden` は子を（角丸込みの）border box に
    /// クリップする。既定は `Visible`。
    pub overflow: OverflowValue,
    /// 切り詰めまでの最大テキスト行数。`None` は無制限。テキスト切り詰めの唯一の
    /// トリガで、これがなければ `text_overflow` は効かない。
    pub max_lines: Option<u32>,
    /// `max_lines` を超えたとき最後の可視行をどう切り詰めるか。
    pub text_overflow: TextOverflowValue,
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub font_style: Option<FontStyleValue>,
    pub text_decoration: Option<TextDecorationValue>,
    /// ポインタカーソルの見た目（ADR-0088）。`None` は `Default` に解決。
    pub cursor: Option<CursorValue>,
    pub z_index: i32,
    /// `register_font` で登録したカスタム font-family 名。
    pub font_family: Option<String>,
    /// ブロックを貫通する周囲の既定テキストスタイル（ADR-0065）。
    pub default_color: Option<Color>,
    pub default_font_size: Option<f32>,
    pub default_font_weight: Option<f32>,
    pub default_font_family: Option<String>,
    /// 擬似状態トランジションの所要時間（ミリ秒、ADR-0089）。既定の `0.0` は
    /// 擬似状態の切り替えを即時適用する。
    pub transition_duration: f32,
    /// 擬似状態トランジション補間中のイージング曲線。
    pub transition_timing: TransitionTimingValue,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            background_color: None,
            opacity: 1.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: None,
            border_style: BorderStyleValue::None,
            box_shadow: Vec::new(),
            overflow: OverflowValue::Visible,
            max_lines: None,
            text_overflow: TextOverflowValue::Clip,
            text_color: None,
            font_size: None,
            font_weight: None,
            font_style: None,
            text_decoration: None,
            cursor: None,
            z_index: 0,
            font_family: None,
            default_color: None,
            default_font_size: None,
            default_font_weight: None,
            default_font_family: None,
            transition_duration: 0.0,
            transition_timing: TransitionTimingValue::Ease,
        }
    }
}

pub(crate) struct Element {
    pub kind: ElementKind,
    pub parent: Option<ElementId>,
    pub children: Vec<ElementId>,
    pub layout_style: taffy::Style,
    pub visual: Visual,
    pub text: Option<String>,
    pub src: Option<String>,
    pub text_layout: Option<crate::element::text::TextLayout>,
    /// レイアウトに上乗せする任意のアフィン変換（kurbo 係数 [a,b,c,d,e,f]）。
    pub transform: Option<[f64; 6]>,
    /// ScrollView 要素のスクロールオフセット（x, y、ピクセル）。
    pub scroll_offset: (f32, f32),
    /// Image 要素のロード済み画像データ（非同期フェッチ後にアダプタが設定）。
    pub src_image: Option<Arc<RenderImage>>,
    /// テキスト入力の編集モデル（TextInput のみ。ADR-0069）。
    pub edit: Option<EditState>,
    /// カーソルを描画すべきか（要素がフォーカスされていれば true）。
    pub cursor_visible: bool,
    /// text_content + preedit の Parley レイアウト。各 render パスで再構築する。
    pub content_layout: Option<crate::element::text::TextLayout>,
    /// スクリーンリーダー向け ARIA ラベル。
    pub aria_label: Option<String>,
    /// ARIA ロール（例 "button" / "listitem"）。None なら暗黙ロールを使う。
    pub role: Option<String>,
    /// Hayate CSS 擬似クラスの上書き（`:hover` / `:active` / `:focus`）。
    pub pseudo_styles: PseudoStyles,
    /// true ならヒットテストとインタラクションを抑止する（ADR-0071）。
    pub disabled: bool,
    /// true なら Selection Region を確立する。配下のテキストをポインタドラッグで
    /// 選択でき、最も近い selectable な祖先で範囲が区切られる（ADR-0097。`disabled`
    /// と同形の閉じた型付きプロパティ）。
    pub selectable: bool,
    /// CSS `user-select` を模した要素ごとの選択可否（ADR-0108）。`None` は Selection
    /// Region 内にあってもこの要素（とサブツリー）を文書選択の範囲とコピーテキスト
    /// から除外する。`Text`（既定）/ `Contains` は選択に参加する。`selectable` とは
    /// 別物で、要素は Region 内にいつつ `user-select: none` で選択を抜けられる。
    pub user_select: crate::element::style::UserSelectValue,
    /// true なら TextInput が改行を受け付ける。Enter はキャレット位置に `\n` を
    /// 挿入し、submit を発火しない。既定 false（単一行）。`disabled` と同形の閉じた
    /// 型付きプロパティ（ADR-0096/0097）。
    pub multiline: bool,
    /// ビューポート条件付きのスタイル上書き。プロパティごとに 1 variant（ADR-0081）。
    pub viewport_variants: Vec<(ViewportCondition, StyleProp)>,
}

/// ScrollView 1 個の Touch 一時インジケータの状態（ADR-0110）。Touch モダリティで
/// コンテンツがスクロール中は表示され、止まるとフェードする。`shown_at_ms` は直近
/// スクロール時のホストクロック（タッチスクロールごとに更新）、`fade` は経過時間から
/// render ごとに再計算する可視率 `[0, 1]`。
#[derive(Clone, Copy, Debug)]
pub(crate) struct TouchScrollIndicator {
    pub shown_at_ms: f64,
    pub fade: f32,
}

/// レイアウト後の要素ごとの完全解決状態。安定 ElementId をキーにする。
/// HTML Mode が SceneGraph を経由せず DOM 要素を更新するのに使う。
#[derive(Clone, Debug)]
pub struct ResolvedElement {
    pub kind: ElementKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub background_color: Option<Color>,
    pub opacity: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub border_color: Option<Color>,
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub z_index: i32,
    pub text: Option<String>,
    pub src: Option<String>,
    /// TextInput 要素の現在値（表示用に text_content + 有効な preedit を結合）。
    pub text_content: Option<String>,
    pub font_family: Option<String>,
    pub aria_label: Option<String>,
    pub role: Option<String>,
}

pub struct ElementTree {
    pub(crate) elements: HashMap<ElementId, Element>,
    pub(crate) root: Option<ElementId>,
    /// レイアウト計算とテキストシェイピングの状態。Taffy / Parley / フォント dirty /
    /// カーソルタイミングを直接触らず、呼び出し側が `commit_frame()` の 1 seam を
    /// 通れるようまとめてある。
    pub(crate) layout: LayoutPass,
    /// dirty 追跡集合とフレーム解決ロジック（ADR-0075）。
    pub(crate) engine: ElementEngine,
    pub(crate) viewport: (f32, f32),
    pub(crate) scene_cache: SceneGraph,
    pub(crate) scene_lowering: SceneLowering,
    pub(crate) event_queue: Vec<Event>,
    /// 横断的 interaction state を所有する deep module（ADR-0122）。この slice では
    /// focus state（`render` のカーソル点滅を進めるための focus 要素、ADR-0032）を
    /// 持ち、`apply_intent` 単一 seam 越しに遷移する。hover / active / press 位置 /
    /// `PointerGesture` 等は後続 slice でここへ移ってくる。
    /// 横断的 interaction state（focus / hover / active / press 位置 /
    /// `PointerGesture` / modality / pointer-pos / cursor / touch scroll）を所有する
    /// deep module（ADR-0122 / #572）。これらの field は `Interaction` 側にあり、
    /// tree からは `self.interaction.*` 越しに借りる。
    pub(crate) interaction: crate::element::interaction::Interaction,
    pub(crate) runtime: DocumentRuntime,
    /// コピー用のプラットフォームクリップボード（ADR-0097）。Platform Adapter が
    /// インストールするまで `None` で、その間コピーは no-op。core は選択テキストを
    /// このトレイト経由で書き込み、具体的なクリップボード API には触れない。
    pub(crate) clipboard: Option<Box<dyn crate::element::clipboard::Clipboard>>,
    /// core が描く選択 chrome（ハイライト / ツールバー）のテーマ。Cupertino 追加が
    /// 加算的になるよう単一の切り替え可能な enum（ADR-0097）。
    pub(crate) selection_chrome_style: crate::element::selection_chrome::SelectionChromeStyle,
    /// 上書き可能な chrome 味付け定数（スクロールバー / 選択など）。既定は正本の
    /// const。dev ビルドでは [`set_chrome_tuning`](Self::set_chrome_tuning) 経由で
    /// `tuning.json` を重ね、JSON 編集と F5 だけで（再ビルドなしに）Chromium/Android
    /// に合わせ込める。
    pub(crate) chrome_tuning: crate::element::chrome_tuning::ChromeTuning,
    /// 静的なツールバーボタンラベルのシェイプ済みレイアウト。layout pass の
    /// フォントコンテキストで一度シェイプしフレーム間で再利用する（ADR-0097）。
    pub(crate) toolbar_label_cache:
        HashMap<crate::element::selection_chrome::ToolbarAction, text::TextLayout>,
}

impl ElementTree {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            root: None,
            layout: LayoutPass::new(),
            engine: ElementEngine::new(),
            viewport: (800.0, 600.0),
            scene_cache: SceneGraph::new(),
            scene_lowering: SceneLowering::default(),
            event_queue: Vec::new(),
            interaction: crate::element::interaction::Interaction::default(),
            runtime: DocumentRuntime::new(),
            clipboard: None,
            selection_chrome_style: crate::element::selection_chrome::SelectionChromeStyle::default(),
            chrome_tuning: crate::element::chrome_tuning::ChromeTuning::default(),
            toolbar_label_cache: HashMap::new(),
        }
    }

    /// 未キャッシュのツールバーボタンラベルを layout pass のフォントコンテキストで
    /// シェイプする（ADR-0097）。ラベルは静的なので一度シェイプして再利用する。
    /// scene の lowering 前に `render` から呼ぶ。
    fn ensure_toolbar_labels(&mut self) {
        use crate::element::selection_chrome::{ToolbarAction, TOOLBAR_LABEL_FONT_SIZE};
        for action in [
            ToolbarAction::Cut,
            ToolbarAction::Copy,
            ToolbarAction::Paste,
            ToolbarAction::SelectAll,
        ] {
            if self.toolbar_label_cache.contains_key(&action) {
                continue;
            }
            let layout = text::build_text_layout(
                &mut self.layout.font_cx,
                &mut self.layout.layout_cx,
                action.label(),
                TOOLBAR_LABEL_FONT_SIZE,
                None,
                None,
                None,
                None,
            );
            self.toolbar_label_cache.insert(action, layout);
        }
    }

    /// キャッシュ済みなら、ツールバーボタンラベルのシェイプ済みレイアウト
    /// （ADR-0097）。scene lowering がラベルのグリフ run 配置に読む。
    pub(crate) fn toolbar_label_layout(
        &self,
        action: crate::element::selection_chrome::ToolbarAction,
    ) -> Option<&text::TextLayout> {
        self.toolbar_label_cache.get(&action)
    }

    /// 選択 chrome のテーマを切り替える（ADR-0097）。既定は Material。Cupertino は
    /// iOS Platform Adapter とともに来る。加算的で、ツールバーのモデルと描画は共有し
    /// スタイルメトリクスのみ異なる。
    pub fn set_selection_chrome_style(
        &mut self,
        style: crate::element::selection_chrome::SelectionChromeStyle,
    ) {
        self.selection_chrome_style = style;
    }

    /// chrome 味付け定数を実行時に上書きする（dev 専用 tuning）。Platform Adapter が
    /// `tuning.json` を解析（serde を所有）し、完全マージ済みの [`ChromeTuning`] を
    /// core へ渡す。欠落/不正な JSON はここに届かないので、フィールドは常に既定値か
    /// 完全な上書きのいずれかを保持する。
    pub fn set_chrome_tuning(&mut self, tuning: crate::element::chrome_tuning::ChromeTuning) {
        self.chrome_tuning = tuning;
    }

    /// scene-build の emit パスが読む、稼働中の chrome 味付け定数。
    pub(crate) fn chrome_tuning(&self) -> &crate::element::chrome_tuning::ChromeTuning {
        &self.chrome_tuning
    }

    /// Platform Adapter のクリップボードをインストールする（ADR-0097）。コピー
    /// ジェスチャ（Cmd/Ctrl+C）は選択テキストをこれ経由で書き込む。なければコピーは
    /// no-op。
    pub fn set_clipboard(&mut self, clipboard: Box<dyn crate::element::clipboard::Clipboard>) {
        self.clipboard = Some(clipboard);
    }

    pub fn interaction_snapshot(&self) -> InteractionSnapshot {
        InteractionSnapshot {
            hovered: self.interaction.hovered_elements.clone(),
            active: self.interaction.active_element,
            focused: self.interaction.focused_element,
        }
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        let new_viewport = (width, height);
        if new_viewport == self.viewport {
            return;
        }
        let old_viewport = self.viewport;
        self.viewport = new_viewport;
        // projection も同じ論理ビューポートを知り、reconcile-sync が実効レイアウト
        // スタイル（base ＋ レイアウト系 variant）を正しく算出できるようにする。
        self.layout.projection.set_layout_viewport(new_viewport);

        // Resize → (shape, visual) の変更集合は 1 モジュールで解決する（ADR-0081）。
        // ここでは返ってきた集合から dirty を立てるだけ。shape 変更は加えて Taffy
        // projection を仕込み、`commit_frame` が再シェイプするようにする。
        let dirty = viewport_resize::resolve_resize(
            self.elements.iter().map(|(id, el)| viewport_resize::ElementResizeInput {
                id: *id,
                base: &el.visual,
                variants: &el.viewport_variants,
            }),
            old_viewport,
            new_viewport,
        );
        for id in dirty.shape {
            self.engine
                .mark_shape_dirty(id, VisualInvalidationReach::Subtree);
            self.layout.projection.mark_dirty(id);
        }
        for id in dirty.visual {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::Subtree);
        }

        // レイアウト系 variant（`display` / `flex-direction` / `width` 等）は visual 差分の
        // `resolve_resize` では拾えない（`apply_visual` がレイアウト prop を捨てるため）。
        // 旧/新ビューポートで実効レイアウトスタイルが変わる要素を別途検出し、projection に
        // 反映して Taffy を再実行させる（ADR-0081）。
        let layout_updates: Vec<(ElementId, taffy::Style)> = self
            .elements
            .iter()
            .filter(|(_, el)| el.viewport_variants.iter().any(|(_, p)| p.is_layout()))
            .filter_map(|(id, el)| {
                let old_style = crate::element::layout_pass::effective_layout_style(
                    &el.layout_style,
                    &el.viewport_variants,
                    old_viewport,
                );
                let new_style = crate::element::layout_pass::effective_layout_style(
                    &el.layout_style,
                    &el.viewport_variants,
                    new_viewport,
                );
                (old_style != new_style).then_some((*id, new_style))
            })
            .collect();
        for (id, style) in layout_updates {
            self.layout.projection.set_style(id, style);
            self.engine
                .mark_shape_dirty(id, VisualInvalidationReach::Subtree);
            self.layout.projection.mark_dirty(id);
        }
    }

    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    pub fn root(&self) -> Option<ElementId> {
        self.root
    }

    pub fn set_root(&mut self, id: ElementId) {
        debug_assert!(self.elements.contains_key(&id), "set_root: unknown id");
        self.root = Some(id);
    }

    pub fn element_create(&mut self, id: u64, kind: ElementKind) -> ElementId {
        let id = ElementId::from_u64(id);
        // 要素種別の UA 既定レイアウトから始める（ADR-0109）。`element_set_style` の
        // 明示プロパティが上に重なり、解決順は 明示 > 要素種別既定 > Taffy 既定。
        let layout_style = kind.base_layout_style();

        let element = Element {
            kind,
            parent: None,
            children: Vec::new(),
            layout_style,
            visual: Visual::default(),
            text: None,
            src: None,
            text_layout: None,
            transform: None,
            scroll_offset: (0.0, 0.0),
            src_image: None,
            edit: if kind == ElementKind::TextInput {
                Some(EditState::default())
            } else {
                None
            },
            cursor_visible: false,
            content_layout: None,
            aria_label: None,
            role: None,
            pseudo_styles: PseudoStyles::default(),
            disabled: false,
            selectable: false,
            // `user-select` を要素種別の UA 既定で初期化する（ADR-0108。`default_cursor`
            // と同じ単一正本テーブル）。フィールドは既に要素ごとの実効値を持つ:
            // `text` / `view` / `scroll-view` / `text-input` は選択可、`image` /
            // `button` は不可。明示の `element_set_user_select` が上書きする。これにより
            // 1 つの core アクセサが I-beam カーソルと選択開始の両方を実効選択可否から
            // 引ける。
            user_select: kind.default_user_select(),
            multiline: false,
            viewport_variants: Vec::new(),
        };
        self.elements.insert(id, element);

        if self.root.is_none() {
            self.root = Some(id);
        }
        id
    }

    pub fn element_set_text(&mut self, id: ElementId, text: &str) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        // text は text-like 要素にのみ宿る（ADR-0058）: `Text`（内容）/ `TextInput`
        // （placeholder）。`view` / `button` / `image` / `scroll-view` はテキストを
        // 子 `text` 要素で持ち、親へ集約しない。非 text 要素への set は無視する
        // （wire 駆動の外部入力なので panic せず防御的に no-op）。
        if !matches!(el.kind, ElementKind::Text | ElementKind::TextInput) {
            return;
        }
        el.text = Some(text.to_string());
        el.text_layout = None;
        self.mark_text_content_dirty(id, VisualInvalidationReach::Subtree);
    }

    pub fn element_set_src(&mut self, id: ElementId, url: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.src = if url.is_empty() {
                None
            } else {
                Some(url.to_string())
            };
            el.src_image = None;
        }
    }

    pub fn element_set_disabled(&mut self, id: ElementId, disabled: bool) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.disabled = disabled;
        }
    }

    /// 要素を Selection Region の境界としてマーク（または解除）する（ADR-0097）。
    pub fn element_set_selectable(&mut self, id: ElementId, selectable: bool) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.selectable = selectable;
        }
    }

    /// 要素の CSS `user-select` 値を設定する（ADR-0108）。`None` は要素とサブツリーを
    /// 文書選択の範囲とコピーテキストから除外する。`Text` / `Contains` は参加する。
    /// Selection Region ルートをマークする
    /// [`element_set_selectable`](Self::element_set_selectable) とは直交。
    pub fn element_set_user_select(
        &mut self,
        id: ElementId,
        value: crate::element::style::UserSelectValue,
    ) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.user_select = value;
        }
    }

    /// TextInput が複数行かどうかを設定する。true なら Enter はキャレット位置に改行を
    /// 挿入し、false（既定）なら Enter が submit を発火する。
    pub fn element_set_multiline(&mut self, id: ElementId, multiline: bool) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.multiline = multiline;
        }
    }

    /// Image 要素のデコード済み画像データを格納する（非同期ロード後にアダプタが呼ぶ）。
    pub fn element_set_image(&mut self, id: ElementId, image: Arc<RenderImage>) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.src_image = Some(image);
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// TextInput 要素の編集可能テキスト内容を置き換える。
    pub fn element_set_text_content(&mut self, id: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set(text);
        }
    }

    /// programmatic な value set（Hayabusa ADR-0007 の「書き・従」経路）。controlled input の
    /// 単一正本は `EditState` なので、signal ミラーからの書き戻しは **現在の確定値と差分があり、
    /// かつ IME 組成中（preedit あり）でないときだけ**適用する。毎キーストロークの echo は
    /// この差分・組成中ガードで no-op に倒れ、preedit / cursor を壊さない。適用したら `true`。
    pub fn element_set_text_content_if_idle(&mut self, id: ElementId, text: &str) -> bool {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            // IME 組成中は書き戻さない（preedit / cursor を保護する）。
            if edit.preedit.is_some() {
                return false;
            }
            // 差分が無ければ何もしない（キーストローク echo の抑止）。
            if edit.text_content == text {
                return false;
            }
            edit.set(text);
            return true;
        }
        false
    }

    /// TextInput の確定済み内容にテキストを追加する。
    pub fn element_append_text_content(&mut self, id: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.append(text);
        }
    }

    /// TextInput の確定済み内容から末尾の Unicode スカラー値を 1 つ削除する。
    pub fn element_backspace(&mut self, id: ElementId) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.backspace();
        }
    }

    /// TextInput 要素の挿入カーソルを表示/非表示にする。
    pub fn element_set_cursor_visible(&mut self, id: ElementId, visible: bool) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.cursor_visible = visible;
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// `id` をフォーカス要素にマークする。`render(timestamp_ms)` が内部でカーソル
    /// 点滅を駆動するのに使う（ADR-0032）。TextInput ターゲットではカーソルも表示し、
    /// フォーカス後の最初のフレームで実線を描く。focus 切替の原始操作（イベントは
    /// 出さない）で、`Interaction` の同名 seam 操作へ委譲する（ADR-0122）。
    pub fn element_focus(&mut self, id: ElementId) {
        let mut interaction = std::mem::take(&mut self.interaction);
        interaction.element_focus(self, id);
        self.interaction = interaction;
    }

    /// `id` のフォーカスを外す（`id` が現在フォーカスされていなければ no-op）。
    /// イベントを出さない原始操作で、`Interaction` の seam 操作へ委譲する。
    pub fn element_blur(&mut self, id: ElementId) {
        let mut interaction = std::mem::take(&mut self.interaction);
        interaction.element_blur(self, id);
        self.interaction = interaction;
    }

    /// 現在フォーカスされている要素（あれば）。
    pub fn focused_element(&self) -> Option<ElementId> {
        self.interaction.focused_element
    }

    /// テキスト入力を受け付けるときのフォーカス要素。すなわちアダプタがソフト
    /// キーボード / IME を出すべきとき（ADR-0102）。タップは当たったもの（ボタン /
    /// 素のテキスト / view、Chromium 互換）をフォーカスするが、編集可能なのは
    /// `text-input` だけ。素のフォーカスでキーボードを出すと全タップで開いてしまう
    /// ため、アダプタはソフトキーボードをこれに紐付ける。
    pub fn focused_text_input(&self) -> Option<ElementId> {
        let id = self.interaction.focused_element?;
        self.elements
            .get(&id)?
            .kind
            .accepts_text_input()
            .then_some(id)
    }

    /// 現フレームのプラットフォーム IME を駆動する（ADR-0069）。ソフトキーボードの
    /// 表示を一度計算し（`text-input` がフォーカスされているとき
    /// （[`focused_text_input`](Self::focused_text_input)）のみ表示、変換候補窓は
    /// キャレットの character bounds に向ける）、アダプタの [`ImeBridge`] へ渡す。
    /// アダプタはこれを反映するだけでキーボード可視性を再導出しないので、編集可否の
    /// ゲートはプラットフォームごとに作らず 1 箇所に集約される。
    pub fn drive_ime(&self, ime: &mut impl ImeBridge) {
        let presentation = match self.focused_text_input() {
            Some(id) => {
                let bounds = self.element_character_bounds(id).unwrap_or(CharacterBounds {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                });
                ImePresentation::Shown { bounds }
            }
            None => ImePresentation::Hidden,
        };
        ime.present(presentation);
    }

    /// 直近入力イベントのモダリティ（ADR-0102）。`:focus-visible` を駆動する
    /// Pointer/Keyboard の軸。[`last_pointer_kind`](Self::last_pointer_kind) とは独立。
    pub fn last_input_modality(&self) -> crate::element::interaction::InputModality {
        self.interaction.last_input_modality
    }

    /// 直近ポインタ操作の物理デバイス。インタラクションごとに保持する。最初の
    /// ポインタイベントがデバイスを報告するまで `Mouse`。
    pub fn last_pointer_kind(&self) -> crate::element::pointer::PointerKind {
        self.interaction.last_pointer_kind
    }

    /// ネイティブフォーカスリングを表示すべきときのフォーカス要素。Chromium の
    /// `:focus-visible` に倣う（ADR-0102）: キーボード駆動のフォーカスは任意の要素を
    /// リングするが、ポインタ駆動のフォーカスはテキスト入力（常に可視キャレットの
    /// コンテキストが要る）はリングし、ボタン等のウィジェットはしない。
    pub fn focus_visible_element(&self) -> Option<ElementId> {
        use crate::element::interaction::InputModality;
        let id = self.interaction.focused_element?;
        let kind = self.elements.get(&id)?.kind;
        let visible = match self.interaction.last_input_modality {
            InputModality::Keyboard => true,
            InputModality::Pointer => kind == ElementKind::TextInput,
        };
        visible.then_some(id)
    }

    /// アクティブ要素を `next` に切り替え、active 状態が変わる全要素の `:active`
    /// 無効化を同一操作内でマークする（ADR-0100）。dirty マークはフィールド書き込みに
    /// 先行するので、`:active` トランジションは切り替え前の見た目（入るとき未 active、
    /// 抜けるとき まだ active）から始まる（ADR-0089）。`active_element` を書くのは
    /// この経路のみで、擬似状態の無効化なしに状態が切り替わることはない。
    pub(crate) fn set_active_element(&mut self, next: Option<ElementId>) {
        if self.interaction.active_element == next {
            return;
        }
        if let Some(prev) = self.interaction.active_element {
            self.mark_pseudo_activation_dirty(prev, PseudoState::Active);
        }
        if let Some(now) = next {
            self.mark_pseudo_activation_dirty(now, PseudoState::Active);
        }
        self.interaction.active_element = next;
        // 押下が終わる/切り替わると保留中タップの起点は無効になる。リリースで
        // クリックを発火する pointer-up 経路が起点を読むので、ここで一括クリアして
        // キャンセルされた押下が古い座標でクリックするのを防ぐ。
        if next.is_none() {
            self.interaction.active_press_pos = None;
        }
    }

    /// 要素の font-family を（名前で）設定する。family は事前に `register_font` で
    /// 登録するか、既定 FontContext で使えるシステムフォントである必要がある。
    pub fn element_set_font_family(&mut self, id: ElementId, family: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.visual.font_family = if family.is_empty() {
                None
            } else {
                Some(family.to_string())
            };
            el.text_layout = None;
            el.content_layout = None;
            self.layout.projection.mark_dirty(id);
        }
    }

    /// スクリーンリーダー向けの ARIA ラベルを設定する。
    pub fn element_set_aria_label(&mut self, id: ElementId, label: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.aria_label = if label.is_empty() {
                None
            } else {
                Some(label.to_string())
            };
        }
    }

    /// ARIA ロール（例 "button" / "listitem" / "img"）を設定する。空文字列でクリア。
    pub fn element_set_role(&mut self, id: ElementId, role: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.role = if role.is_empty() {
                None
            } else {
                Some(role.to_string())
            };
        }
    }

    /// 生バイトからカスタムフォントを family 名付きで登録する。登録後はその名前を
    /// `element_set_font_family` で使える。
    pub fn register_font(&mut self, family_name: &str, bytes: Vec<u8>) {
        // 要求された名前で登録し（明示の `font-family` 用）、バンドル既定の後ろに
        // クラスタごとのフォールバックとして組み込む。既定 family のエイリアスには
        // してはならない。そうするとバンドルの日本語カバー face を覆い隠し、Latin/emoji
        // フォールバックが取得された瞬間に全 CJK が tofu になる（デプロイ Pages の
        // カスケード）。`text::register_collection_font` 参照。
        text::register_collection_font(
            &mut self.layout.font_cx.collection,
            family_name,
            Arc::new(bytes),
        );

        self.layout.font_fetches.mark_loaded(family_name);
        self.engine.mark_fonts_dirty();
    }

    /// プラットフォームアダプタによる `family` のフェッチが失敗したことを報告する。
    /// これがないと、`FetchFont` で要求された family が pending 集合に永久に残って
    /// 再要求されず、一度の一時的 CDN エラー（新規デプロイ時の 403/429 など）で
    /// フォントが恒久的に欠落してしまう。
    ///
    /// family を再試行するなら `true` を返す。core はフォントを dirty にし、次フレームで
    /// 再シェイプ・欠落再検出・`FetchFont` 再発行する。有限のリトライ予算を使い切ると
    /// `false` を返す。family は諦められ再要求されない（暴走ログや連打なし）。
    pub fn font_fetch_failed(&mut self, family: &str) -> bool {
        use crate::element::font_fetch::FailureOutcome;
        match self.layout.font_fetches.mark_failed(family) {
            FailureOutcome::WillRetry => {
                self.engine.mark_fonts_dirty();
                true
            }
            FailureOutcome::GaveUp => false,
        }
    }

    /// フォントファイル自身に埋め込まれた family 名を使って生バイトから登録する。
    /// WIT の `element-load-font` エクスポートを支える。
    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) {
        let blob = Blob::new(Arc::new(bytes));
        self.layout.font_cx.collection.register_fonts(blob, None);
    }

    /// TextInput のキャレット/選択を確定テキスト座標（`anchor`/`focus` バイト
    /// オフセット）で設定する。ソフトキーボード/IME の絶対状態（ADR-0094）から
    /// 報告される selection をコアへ反映し、preedit/確定をキャレット位置に置くために使う。
    pub fn element_set_selection(&mut self, id: ElementId, anchor: usize, focus: usize) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_selection(anchor, focus);
        }
    }

    /// TextInput の IME preedit（変換中・未確定）を設定する。
    pub fn element_set_preedit(&mut self, id: ElementId, preedit: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit(preedit);
        }
    }

    /// IME preedit を変換クラスのフォーマット範囲（ADR-0102）とともに設定する。
    /// Canvas Mode がクラスごとの変換下線を描けるようにする EditContext
    /// `textformatupdate` 経路。
    pub fn element_set_preedit_with_clauses(
        &mut self,
        id: ElementId,
        preedit: &str,
        clauses: Vec<crate::element::edit_state::CompositionClause>,
    ) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit_with_clauses(preedit, clauses);
        }
    }

    /// 現在の preedit テキストを text_content へ確定し、preedit をクリアする。
    pub fn element_commit_preedit(&mut self, id: ElementId) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.commit_preedit();
        }
    }

    /// IME 変換確定: `text` をキャレット位置に確定する（アクティブな preedit があれば
    /// 置換）。増分コマンド経路（`ImeAction::CommitText`、ADR-0117）の適用先で、
    /// web 経路の `on_composition_end` と同じ `EditState::finish_composition` を駆動する。
    pub fn element_finish_composition(&mut self, id: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.finish_composition(text);
        }
    }

    /// キャレット直前の 1 グラフェムを削除する（キャレット対応の backspace）。末尾固定の
    /// [`Self::element_backspace`] と違い、確定テキスト中央のキャレットからも正しく削る。
    /// 増分コマンド経路（`ImeAction::DeleteBackward`、ADR-0117）の適用先。
    pub fn element_delete_backward(&mut self, id: ElementId) {
        use crate::element::edit_state::{Direction, EditIntent, Granularity};
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.apply(EditIntent::Delete {
                granularity: Granularity::Grapheme,
                direction: Direction::Backward,
            });
        }
    }

    /// 貼り付けテキストを TextInput へ届ける。有効な preedit を確定し、貼り付け
    /// テキストを追加し、TextInput イベントをキューする。非 TextInput 要素では no-op。
    pub fn element_paste(&mut self, id: ElementId, text: &str) {
        let pasted = text.to_string();
        let el = match self.elements.get_mut(&id) {
            Some(e) if e.kind == ElementKind::TextInput => e,
            _ => return,
        };
        let Some(edit) = el.edit.as_mut() else {
            return;
        };
        if !edit.paste(&pasted) {
            return;
        }
        // 貼り付け断片ではなく確定後の全文を載せる（on_text_input と同型、ADR-0069 / #474）。
        let value = self.element_get_text_content(id);
        self.dispatch_event(
            DocumentEventKind::TextInput,
            Event::TextInput {
                target_id: id,
                text: value,
            },
        );
    }

    /// TextInput の結合表示テキスト（text_content + 有効な preedit）を返す。
    pub fn element_get_text_content(&self, id: ElementId) -> String {
        self.elements
            .get(&id)
            .and_then(|el| el.edit.as_ref())
            .map(|edit| edit.display_text())
            .unwrap_or_default()
    }

    /// テキスト入力の現在の編集選択を正規化バイト範囲 `(start, end)` で返す。
    /// 要素が編集可能なテキスト入力でない、または選択がキャレットに collapse して
    /// いるときは `None`（ADR-0097）。
    pub fn element_text_selection(&self, id: ElementId) -> Option<(usize, usize)> {
        self.elements
            .get(&id)
            .and_then(|el| el.edit.as_ref())
            .and_then(|edit| edit.selection_range())
    }

    /// テキスト入力のキャレット（選択の focus）を `text_content` 内へのバイト
    /// オフセットで返す。要素が編集可能なテキスト入力でなければ `None`。キャレット
    /// 移動 intent の観測可能な出力（ADR-0103）: collapse したキャレットは
    /// `element_text_selection` が `None` でも位置を報告する。
    pub fn element_caret_byte_index(&self, id: ElementId) -> Option<usize> {
        self.elements
            .get(&id)
            .and_then(|el| el.edit.as_ref())
            .map(|edit| edit.cursor_byte_index)
    }

    /// テキスト入力の有効な IME 変換下線を、表示テキストのバイト範囲とその weight で
    /// 返す（ADR-0102）。変換中でなければ空。`element_set_preedit_with_clauses` の
    /// クエリ側。
    pub fn element_composition_underlines(
        &self,
        id: ElementId,
    ) -> Vec<(usize, usize, crate::element::edit_state::CompositionUnderline)> {
        self.elements
            .get(&id)
            .and_then(|el| el.edit.as_ref())
            .map(|edit| edit.composition_underlines())
            .unwrap_or_default()
    }

    /// 要素に 2D アフィン変換を設定する（kurbo 係数 6 つ [a,b,c,d,e,f]）。None で
    /// クリア。変換はレイアウト座標の上に適用される。
    pub fn element_set_transform(&mut self, id: ElementId, matrix: Option<[f64; 6]>) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.transform = matrix;
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// ScrollView 要素のスクロールオフセットをプログラムから設定する。
    pub fn element_set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
        let mut scrolled_scroll_view = false;
        if let Some(el) = self.elements.get_mut(&id) {
            scrolled_scroll_view = el.scroll_offset != (x, y) && el.kind == ElementKind::ScrollView;
            el.scroll_offset = (x, y);
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
        // Touch モダリティでのスクロールは一時インジケータを再表示する。Mouse/Pen の
        // 操作可能つまみに対する Touch 側の相当物（ADR-0110）。刻みはホストクロックを
        // 所有する render に委ねる。
        if scrolled_scroll_view
            && self.interaction.last_pointer_kind == crate::element::pointer::PointerKind::Touch
        {
            self.interaction.touch_scroll_pending.insert(id);
        }
    }

    /// `id` の Touch 一時インジケータの現在の可視率 `[0, 1]`。稼働中のインジケータが
    /// なければ `0.0`（ADR-0110）。スクロールバーオーバーレイの lowering が
    /// インジケータ不透明度のスケールに読む。値は render ごとに
    /// （`advance_touch_scroll_indicators`）計算されるので lowering は独自クロックを
    /// 持たない。
    pub(crate) fn touch_scroll_indicator_opacity(&self, id: ElementId) -> f32 {
        self.interaction.touch_scroll_indicators
            .get(&id)
            .map_or(0.0, |i| i.fade)
    }

    /// 要素の現在のスクロールオフセットを読む。
    pub fn element_get_scroll_offset(&self, id: ElementId) -> (f32, f32) {
        self.elements
            .get(&id)
            .map_or((0.0, 0.0), |e| e.scroll_offset)
    }

    /// 直近 render パスの絶対レイアウト矩形 (x, y, w, h) を返す。縮約レイアウト
    /// インターフェースのジオメトリ照会側。
    pub fn element_layout_rect(&self, id: ElementId) -> Option<(f32, f32, f32, f32)> {
        self.layout.geometry(id)
    }

    /// ScrollView の全子孫を囲む寸法（コンテンツサイズ）を返す。値は要素自身の
    /// 左上隅を基準とする。
    pub fn element_content_size(&self, id: ElementId) -> (f32, f32) {
        let (ex, ey, _, _) = match self.layout.geometry(id) {
            Some(r) => r,
            None => return (0.0, 0.0),
        };
        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;
        self.accumulate_content_bounds(id, ex, ey, &mut max_x, &mut max_y);
        (max_x, max_y)
    }

    /// ScrollView の CSS 準拠の最大スクロールオフセット `(max_x, max_y)`。軸ごとに 0 で
    /// 下限を取る — ブラウザの `scroll{Width,Height} − client{Width,Height}` 範囲。
    /// 末尾までスクロールすると、DOM モード（ネイティブ `scrollTop`）と同様に最後の
    /// 子の下にスクロールビュー自身の末尾 padding を露出させねばならない（Semantics
    /// Parity）。
    ///
    /// `element_content_size` は子孫を border-box 上端から測るのでスクロールビュー
    /// 自身の下/右 padding を含まず、`element_layout_rect` は border box なので
    /// ビューポートとしては border を過剰計上する。どちらのギャップもスクロール
    /// ビュー自身の末尾インセット（padding + border）なので、足し戻すと content box
    /// で測った `child_extent − content_box` になり padding と border が相殺する
    /// （正しい範囲）。素の border box を引く（旧 `(content − view).max(0)`）と
    /// ちょうど `padding_end + border_end` だけ過少スクロールになり、その固定長が
    /// 到達不能になっていた。
    ///
    /// ホイールクランプ、タッチのラバーバンド（`canvas.rs`）、scroll-into-view
    /// （`accessibility.rs`）の単一正本。
    pub fn element_scroll_max_offset(&self, id: ElementId) -> (f32, f32) {
        let (content_w, content_h) = self.element_content_size(id);
        let (_, _, view_w, view_h) = self.layout.geometry(id).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let (end_x, end_y) = self.scroll_view_end_insets(id);
        (
            (content_w - view_w + end_x).max(0.0),
            (content_h - view_h + end_y).max(0.0),
        )
    }

    /// 直近レイアウトパスにおける `id` の右/下（padding + border）インセット。
    /// `element_content_size`（border-box 上端基準）と `element_layout_rect`
    /// （border box）がスクロール範囲から落とす可動量を回復する。`id` が未レイアウト
    /// なら 0。
    fn scroll_view_end_insets(&self, id: ElementId) -> (f32, f32) {
        let Some(node) = self.layout.projection.node_id(id) else {
            return (0.0, 0.0);
        };
        let Ok(box_layout) = self.layout.projection.taffy.layout(node) else {
            return (0.0, 0.0);
        };
        (
            box_layout.padding.right + box_layout.border.right,
            box_layout.padding.bottom + box_layout.border.bottom,
        )
    }

    fn accumulate_content_bounds(
        &self,
        id: ElementId,
        origin_x: f32,
        origin_y: f32,
        max_x: &mut f32,
        max_y: &mut f32,
    ) {
        let el = match self.elements.get(&id) {
            Some(e) => e,
            None => return,
        };
        for &child in &el.children {
            if let Some((cx, cy, cw, ch)) = self.layout.geometry(child) {
                *max_x = max_x.max(cx - origin_x + cw);
                *max_y = max_y.max(cy - origin_y + ch);
                // 自身のオーバーフローをクリップする子（ネストした ScrollView や
                // `overflow: hidden` の箱 — `scene_build` が Clip で包むのと同じ要素）は
                // 子孫を自分の箱に閉じ込める。そのクリップされた overflow は子の私的
                // コンテンツであり我々のものではない。再帰すると我々のコンテンツサイズが
                // 膨らみ、実コンテンツの先の空白までスクロール可能になってしまう。
                // クリップを越えて降りないことで寄与を子の箱に収める。
                if !self.clips_overflow(child) {
                    self.accumulate_content_bounds(child, origin_x, origin_y, max_x, max_y);
                }
            }
        }
    }

    /// `id` がオーバーフローをクリップし、子孫が祖先のスクロール可能コンテンツに
    /// 寄与しないかどうか。`scene_build` の Clip ラッパー条件を反映する（ScrollView は
    /// 常にクリップ、それ以外は `overflow: hidden`）。
    fn clips_overflow(&self, id: ElementId) -> bool {
        self.elements.get(&id).is_some_and(|el| {
            el.kind == ElementKind::ScrollView
                || el.visual.overflow == crate::element::style::OverflowValue::Hidden
        })
    }

    pub fn element_set_style(&mut self, id: ElementId, props: &[StyleProp]) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        let mut layout_changed = false;
        let mut text_dirty = false;
        for prop in props {
            // `overflow` は二面性を持つ: 子をクリップし（Visual）、かつ箱を
            // スクロールコンテナにする。後者は flex の自動最小サイズを 0 にし、兄弟を
            // あふれさせる代わりに縮小させる（Layout）。両面を適用する。視覚無効化の
            // 経路に留める（`layout_changed` は立てない）ので、overflow 単独の変更でも
            // `classify_style_props` 経由で scene を再クリップする。レイアウト効果は
            // `set_overflow` で直接マークする。（実レイアウトプロパティも含むバッチは
            // レイアウトを再実行し、どのみち scene を再構築してクリップ用に
            // `visual.overflow` を読み直す。）
            if let StyleProp::Overflow(v) = prop {
                apply_visual(&mut el.visual, prop, &mut text_dirty);
                self.layout.set_overflow(id, &mut el.layout_style, *v);
                continue;
            }
            // 縮約レイアウトインターフェースの片側: レイアウト seam が変換 +
            // Taffy set + マークを担う。非レイアウトプロパティは Visual へ落ちる。
            if self.layout.set_layout_prop(id, &mut el.layout_style, prop) {
                layout_changed = true;
            } else {
                apply_visual(&mut el.visual, prop, &mut text_dirty);
            }
        }
        if text_dirty {
            el.text_layout = None;
        }
        if layout_changed {
            // `set_layout_prop` は base を直接 projection へ流す。レイアウト系 variant を
            // 持つ要素では、その上に一致 variant を重ねた実効スタイルへ差し替える
            // （reconcile-sync が走らない純粋スタイル変更でも variant を保つ・ADR-0081）。
            if el.viewport_variants.iter().any(|(_, p)| p.is_layout()) {
                let style = crate::element::layout_pass::effective_layout_style(
                    &el.layout_style,
                    &el.viewport_variants,
                    self.viewport,
                );
                self.layout.projection.set_style(id, style);
                self.layout.projection.mark_dirty(id);
            }
            return;
        }
        let change = self.classify_style_props(id, props);
        self.apply_change_at(id, change);
    }

    /// スタイル変更中の全非レイアウトプロパティの無効化を、要素のコンテキストに
    /// 照らしてマージする（*何を*）。空/全レイアウトのリストは scene のみの自己
    /// 再描画にフォールバックする。
    fn classify_style_props(
        &self,
        id: ElementId,
        props: &[StyleProp],
    ) -> Change {
        let ctx = self.element_context(id);
        props
            .iter()
            .filter(|p| !p.is_layout())
            .map(|p| visual_invalidation::classify(p, ctx))
            .reduce(Change::merge)
            .unwrap_or_else(Change::visual_self_only)
    }

    /// 分類済みの `Change` を、単一のルーティング seam を通して稼働中の dirty 集合へ
    /// 適用する（ADR-0099）。*どの要素か*（トポロジ: shape 変更は囲む shaping unit へ
    /// 再ターゲット）を解決し、`dirty_kind → sinks` テーブルを唯一知る `route_change` へ
    /// `Change` を渡す。呼び出し側が engine / projection のマークを手配線することはない。
    fn apply_change_at(&mut self, id: ElementId, change: Change) {
        let target = match change.dirty_kind {
            // shape 変更は shaping unit をマークする: 囲む IFC root、または Taffy box を
            // 持つときは要素自身。どちらも持たない（切り離された/箱なしの）ノードは
            // 再シェイプ対象を持たない。
            DirtyKind::Shape => self.shape_target(id),
            DirtyKind::Visual | DirtyKind::Structure => Some(id),
        };
        if let Some(target) = target {
            let mut sink = EngineProjectionSink {
                engine: &mut self.engine,
                projection: &mut self.layout.projection,
            };
            visual_invalidation::route_change(&mut sink, target, change);
        }
    }

    /// shape 変更の dirty マークを担う要素: 囲む IFC root、または Taffy box を持つ
    /// ときは要素自身。純粋なトポロジで、*何か*（これが shape 変更であること）は
    /// すでに `classify` が決めている。
    fn shape_target(&self, id: ElementId) -> Option<ElementId> {
        if let Some(root) = ifc_root(&self.elements, id) {
            Some(root)
        } else if self.layout.projection.has_node(id) {
            Some(id)
        } else {
            None
        }
    }

    /// ビューポート条件付きのスタイル上書きを追加する（ADR-0081）。
    ///
    /// 同一プロパティの複数 variant は宣言順に保持される。
    /// `element_effective_visual` は一致する全 variant を適用し、後のエントリが勝つ
    /// （CSS `@media` カスケード）。
    pub fn element_set_style_variant(
        &mut self,
        id: ElementId,
        condition: ViewportCondition,
        prop: StyleProp,
    ) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        let is_layout = prop.is_layout();
        el.viewport_variants.push((condition, prop));
        // レイアウト系 variant は実効レイアウトスタイルを今すぐ projection へ反映する。
        // 現ビューポートで条件が成立していれば Taffy に効き、成立していなくても base が
        // 再確定するだけ（後の set_viewport で flip を拾う）。ビジュアル系は従来経路。
        if is_layout {
            let style = crate::element::layout_pass::effective_layout_style(
                &el.layout_style,
                &el.viewport_variants,
                self.viewport,
            );
            self.layout.projection.set_style(id, style);
            self.engine
                .mark_shape_dirty(id, VisualInvalidationReach::Subtree);
            self.layout.projection.mark_dirty(id);
        }
    }

    /// 継承可能なスタイルプロパティを 1 つ以上 unset し、「親から継承」に戻す。
    pub fn element_unset_style(&mut self, id: ElementId, kinds: &[StylePropKind]) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        let mut text_dirty = false;
        for kind in kinds {
            match kind {
                StylePropKind::Color => {
                    el.visual.text_color = None;
                }
                StylePropKind::FontSize => {
                    el.visual.font_size = None;
                    el.text_layout = None;
                    text_dirty = true;
                }
                StylePropKind::FontFamily => {
                    el.visual.font_family = None;
                    el.text_layout = None;
                    text_dirty = true;
                }
                StylePropKind::FontWeight => {
                    el.visual.font_weight = None;
                    el.text_layout = None;
                    text_dirty = true;
                }
            }
        }
        if text_dirty {
            self.mark_text_content_dirty(id, VisualInvalidationReach::Subtree);
        }
    }

    pub fn element_append_child(&mut self, parent: ElementId, child: ElementId) {
        if !self.elements.contains_key(&parent) || !self.elements.contains_key(&child) {
            return;
        }
        self.detach_from_current_parent(child);
        self.elements.get_mut(&parent).unwrap().children.push(child);
        self.elements.get_mut(&child).unwrap().parent = Some(parent);
        self.mark_child_attachment_dirty(parent, child);
    }

    pub fn element_insert_before(
        &mut self,
        parent: ElementId,
        child: ElementId,
        before: ElementId,
    ) {
        if !self.elements.contains_key(&parent)
            || !self.elements.contains_key(&child)
            || !self.elements.contains_key(&before)
        {
            return;
        }
        self.detach_from_current_parent(child);
        let index = match self.elements[&parent]
            .children
            .iter()
            .position(|&c| c == before)
        {
            Some(i) => i,
            None => {
                // `before` が `parent` の子でない場合はフォールバックで末尾追加する。
                self.element_append_child(parent, child);
                return;
            }
        };
        self.elements
            .get_mut(&parent)
            .unwrap()
            .children
            .insert(index, child);
        self.elements.get_mut(&child).unwrap().parent = Some(parent);
        self.mark_child_attachment_dirty(parent, child);
    }

    pub fn element_remove(&mut self, id: ElementId) {
        if !self.elements.contains_key(&id) {
            return;
        }
        self.detach_from_current_parent(id);
        // サブツリーを再帰的に削除する。
        let mut stack = vec![id];
        let mut to_remove = Vec::new();
        while let Some(node) = stack.pop() {
            to_remove.push(node);
            if let Some(el) = self.elements.get(&node) {
                stack.extend(el.children.iter().copied());
            }
        }
        if let Some(root) = self.root {
            self.engine.mark_structure_dirty(root);
        }
        for node in to_remove.into_iter().rev() {
            self.elements.remove(&node);
            self.runtime.remove_element_listeners(node);
            // 状態遷移ではなく解体: 要素は消え、サブツリー全体は既に
            // structure-dirty（上）なので、無効化すべき擬似スタイルは残っていない。
            // だからこれらは、稼働中の状態切り替えを守る atomic な set/clear seam
            // （ADR-0100）を通さず、インタラクションフィールドを直接クリアする。
            if self.interaction.focused_element == Some(node) {
                self.interaction.focused_element = None;
                self.layout.last_cursor_toggle_ms = None;
            }
            self.interaction.hovered_elements.remove(&node);
            if self.interaction.active_element == Some(node) {
                self.interaction.active_element = None;
            }
        }
        if self.root == Some(id) {
            self.root = None;
        }
    }

    /// ポインタ下の最深ヒットから CSS `:hover` 集合を更新する。イベントディスパッチ
    /// 用に `(entered, left)` を返す。
    pub fn update_pointer_hover(&mut self, deepest_hit: Option<ElementId>) -> (Vec<ElementId>, Vec<ElementId>) {
        let next = match deepest_hit {
            Some(hit) => hover_set_for_hit(&self.elements, hit),
            None => HashSet::new(),
        };
        let (entered, left) = diff_hover_sets(&self.interaction.hovered_elements, &next);
        for id in &entered {
            self.mark_pseudo_activation_dirty(*id, PseudoState::Hover);
        }
        for id in &left {
            self.mark_pseudo_activation_dirty(*id, PseudoState::Hover);
        }
        self.interaction.hovered_elements = next;
        (entered, left)
    }

    /// HTML `mouseenter` 経路: 単一要素を hover にマークする（親は子の上でも hover を
    /// 保つ）。`:hover` 無効化は集合の切り替えと同一操作に乗る（ADR-0100）ので、
    /// HTML hover 経路は要素の `:hover` 見た目を再 lowering せずに hover 状態を変えら
    /// れない。集合が変わったかどうかを返す。
    pub fn hover_enter_element(&mut self, id: ElementId) -> bool {
        if self.interaction.hovered_elements.insert(id) {
            self.mark_pseudo_activation_dirty(id, PseudoState::Hover);
            true
        } else {
            false
        }
    }

    /// HTML `mouseleave` 経路: 離れた要素の hover をクリアし、`:hover` 無効化を同一
    /// 操作内でマークする（ADR-0100）。集合が変わったかどうかを返す。
    pub fn hover_leave_element(&mut self, id: ElementId) -> bool {
        if self.interaction.hovered_elements.remove(&id) {
            self.mark_pseudo_activation_dirty(id, PseudoState::Hover);
            true
        } else {
            false
        }
    }

    pub fn element_set_pseudo_style(&mut self, id: ElementId, state: PseudoState, props: &[StyleProp]) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        let slot = el.pseudo_styles.props_mut(state);
        for prop in props {
            if prop.is_layout() {
                continue;
            }
            pseudo_state::upsert_style_prop(slot, prop);
        }
        let reach = self.classify_style_props(id, props).reach;
        self.apply_change_at(
            id,
            Change {
                dirty_kind: DirtyKind::Visual,
                reach,
            },
        );
    }

    pub fn element_unset_pseudo_style(
        &mut self,
        id: ElementId,
        state: PseudoState,
        kinds: &[StylePropKind],
    ) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        for kind in kinds {
            pseudo_state::unset_pseudo_prop(&mut el.pseudo_styles, state, *kind);
        }
        self.apply_change_at(
            id,
            Change {
                dirty_kind: DirtyKind::Visual,
                reach: VisualInvalidationReach::Subtree,
            },
        );
    }

    pub fn element_get_text(&self, id: ElementId) -> String {
        self.elements
            .get(&id)
            .and_then(|e| e.text.clone())
            .unwrap_or_default()
    }

    pub fn element_kind(&self, id: ElementId) -> Option<ElementKind> {
        self.elements.get(&id).map(|e| e.kind)
    }

    pub fn element_parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(&id).and_then(|e| e.parent)
    }

    /// 直近レイアウトパスで `id` が Taffy ノードへ投影されたかどうか。
    #[doc(hidden)]
    pub fn element_has_taffy_node(&self, id: ElementId) -> bool {
        self.layout.projection.has_node(id)
    }

    /// `root` とその子孫の要素 id（pre-order）。未知のときは空。
    pub fn subtree_element_ids(&self, root: ElementId) -> Vec<ElementId> {
        if !self.elements.contains_key(&root) {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            out.push(node);
            if let Some(el) = self.elements.get(&node) {
                stack.extend(el.children.iter().copied());
            }
        }
        out
    }

    /// レイアウトを実行し、要素ツリーを scene graph へ lowering して返す。
    ///
    /// `timestamp_ms` は単調増加のホストクロック（例 `performance.now()`）。cursor-tick
    /// 関数をホストへ露出せずにフォーカス中 TextInput のカーソル点滅を駆動する
    /// （ADR-0032）。
    pub fn render(&mut self, timestamp_ms: f64) -> &SceneGraph {
        if self.root.is_some() {
            if let Some(id) = self.layout.advance_cursor_blink(
                &mut self.elements,
                self.interaction.focused_element,
                timestamp_ms,
            ) {
                self.engine
                    .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
            }
        }
        self.advance_touch_scroll_indicators(timestamp_ms);
        let mut dirty = collect_lowering_dirty(
            self,
            &self.engine.structure_dirty,
            &self.engine.shape_dirty,
            &self.engine.shape_lowering_reach,
            &self.engine.visual_dirty,
            self.engine.fonts_dirty,
        );
        self.commit_frame();
        // `commit_frame` がレイアウトを再実行した。box ジオメトリが変わった要素を
        // 本フレームの lowering 集合へ畳み込み、リフローしただけで他は綺麗な箱
        // （成長した祖先・押された兄弟）が古いジオメトリを描かず再 lowering される
        // ようにする。差分は新しいレイアウトキャッシュができて初めて分かるので
        // commit 後に、かつ `scene_build::update` が `dirty` を消費する前に行う。
        let geometry_dirty = self.engine.drain_layout_geometry_dirty();
        let _ = self.engine.drain_visual_dirty();
        let _ = self.engine.drain_shape_lowering_reach();
        for id in geometry_dirty {
            visual_invalidation::apply_visual_invalidation(
                self,
                id,
                VisualInvalidationReach::SelfOnly,
                &mut dirty.elements,
                &mut dirty.z_index_reorder_parents,
            );
        }
        // lowering が読む前にツールバーラベルをシェイプする（ADR-0097）。
        self.ensure_toolbar_labels();
        let mut scene_cache = std::mem::take(&mut self.scene_cache);
        let mut scene_lowering = std::mem::take(&mut self.scene_lowering);
        scene_build::update(self, &mut scene_cache, &mut scene_lowering, dirty, timestamp_ms);
        // トランジションは lowering seam で進む。まだ補間中の要素は visual-dirty の
        // まま保ち、次フレームで再 lowering して進める。最後のトラックが本フレームで
        // 落ち着くと要素は再マークされず、フレームループは静止する（ADR-0086/0093）。
        for id in scene_lowering.active_transition_ids() {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
        self.scene_cache = scene_cache;
        self.scene_lowering = scene_lowering;
        &self.scene_cache
    }

    /// 本フレームの Touch 一時スクロールバーインジケータを進める（ADR-0110）。前回
    /// render 以降 Touch でスクロールした ScrollView はインジケータを全可視で再表示し、
    /// `now_ms` を fade クロックとして刻む。稼働中の各インジケータは経過時間から可視率を
    /// 再計算し（hold 窓の間は full、fade 窓で 0 へ低下）、0 に達すると破棄される。まだ
    /// アニメ中の各 ScrollView は visual-dirty に保ち、次フレームで再 lowering して fade を
    /// 進める。最後の 1 つが落ち着くとフレームループは静止する（進行中トランジション /
    /// カーソル点滅の刻みと同形、ADR-0086/0093/0032）。
    fn advance_touch_scroll_indicators(&mut self, now_ms: f64) {
        for id in std::mem::take(&mut self.interaction.touch_scroll_pending) {
            self.interaction.touch_scroll_indicators.insert(
                id,
                TouchScrollIndicator {
                    shown_at_ms: now_ms,
                    fade: 1.0,
                },
            );
        }
        let mut dirty = Vec::new();
        self.interaction.touch_scroll_indicators.retain(|&id, ind| {
            let elapsed = now_ms - ind.shown_at_ms;
            ind.fade = scene_build::touch_scroll_indicator_fade(elapsed);
            // インジケータが稼働中は箱を再 lowering し続け、fade をフレームごとに
            // 進める。消えるフレームでももう一度行う。
            dirty.push(id);
            ind.fade > 0.0
        });
        for id in dirty {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// dirty 状態を解決しレイアウトを確定する（`LayoutPass::run()` 相当、ADR-0075）:
    /// Taffy projection の reconcile、Parley テキストシェイピング、レイアウトキャッシュ
    /// の更新。scene graph の lowering やカーソル点滅の前進は行わない。
    pub fn commit_frame(&mut self) {
        if let Some(root) = self.root {
            self.engine.resolve(
                &mut self.layout,
                &mut self.elements,
                root,
                self.viewport,
                &mut self.event_queue,
            );
        }
    }

    pub fn scene_graph(&self) -> &SceneGraph {
        &self.scene_cache
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.event_queue)
    }

    pub fn register_listener(
        &mut self,
        element_id: ElementId,
        kind: DocumentEventKind,
    ) -> ListenerId {
        self.runtime.register_listener(element_id, kind)
    }

    pub fn dispatch_event(&mut self, kind: DocumentEventKind, event: Event) {
        let mut path = Vec::new();
        let mut node = document_runtime::event_target(&event);
        while let Some(id) = node {
            path.push(id);
            if !kind.bubbles() {
                break;
            }
            node = self.element_parent(id);
        }
        self.runtime.dispatch_to_path(&path, kind, event);
    }

    pub fn poll_deliveries(&mut self) -> Vec<EventDelivery> {
        self.runtime.poll_deliveries()
    }

    /// `id` から最も近い ScrollView 祖先（自身を含む）。なければ None。ホイール経路の連鎖開始点と
    /// タッチジェスチャのロック（ADR-0082）が共有する単一の真実で、各 Platform Adapter が祖先走査を
    /// 再実装せずにこの公開シームへ委譲できる。
    pub fn nearest_scroll_view(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.element_kind(id) == Some(ElementKind::ScrollView) {
                return Some(id);
            }
            id = self.element_parent(id)?;
        }
    }

    /// `hit` の祖先 ScrollView にブラウザ風のスクロールチェーンでホイール差分を適用する。
    ///
    /// 最も近い ScrollView から始め、各軸はコンテンツ境界まで差分を消費する。未消費の
    /// 残りは root まで次の祖先 ScrollView へ伝播する。
    pub fn apply_wheel_delta(
        &mut self,
        hit: ElementId,
        delta_x: f32,
        delta_y: f32,
    ) -> Option<ElementId> {
        let first_sv = self.nearest_scroll_view(hit)?;
        let mut current_sv = first_sv;
        let mut remaining_x = delta_x;
        let mut remaining_y = delta_y;
        let mut any_applied = false;

        loop {
            if remaining_x.abs() < 1e-6 && remaining_y.abs() < 1e-6 {
                break;
            }

            let (ox, oy) = self.element_get_scroll_offset(current_sv);
            let (max_x, max_y) = self.element_scroll_max_offset(current_sv);
            let new_x = (ox + remaining_x).clamp(0.0, max_x);
            let new_y = (oy + remaining_y).clamp(0.0, max_y);
            let consumed_x = new_x - ox;
            let consumed_y = new_y - oy;

            if consumed_x.abs() > 1e-6 || consumed_y.abs() > 1e-6 {
                self.element_set_scroll_offset(current_sv, new_x, new_y);
                any_applied = true;
            }

            remaining_x -= consumed_x;
            remaining_y -= consumed_y;

            match next_ancestor_scroll_view(self, current_sv) {
                Some(next) => current_sv = next,
                None => break,
            }
        }

        if any_applied {
            Some(first_sv)
        } else {
            None
        }
    }

    /// 送出キューにイベントを追加する。
    pub fn push_event(&mut self, event: Event) {
        self.event_queue.push(event);
    }

    /// レイアウトパスが少なくとも 1 回完了していれば（layout_cache に値がある）true。
    pub fn has_layout(&self) -> bool {
        self.layout.has_geometry()
    }

    /// Z-Order の単一正本。`id` の子兄弟を **paint order**（z 昇順・同 z は
    /// document 順で安定 = 後勝ち）で返す。
    ///
    /// 描画（`scene_build`）はこの順で前方反復し、hit-test は `.rev()` で最前面から
    /// 走る。「hit-test = paint の逆順」を構造的に保証するため、Z-Order の順序解決は
    /// この 1 メソッドに集約する。`resolved_elements` / HTML 経路は意図的にこの seam を
    /// 通さず document order を保つ（CSS が stacking、将来の a11y 読み上げ順は document
    /// order）。ADR-0021 / ADR-0060。
    pub fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
        let mut children = match self.elements.get(&id) {
            Some(el) => el.children.clone(),
            None => return Vec::new(),
        };
        // 安定ソート: 同 z は元の document 順を保持する。
        children.sort_by_key(|cid| self.elements.get(cid).map_or(0, |c| c.visual.z_index));
        children
    }

    /// IME 変換候補窓のための character bounds（ADR-0069）。事前のレイアウトが必要。
    pub fn element_character_bounds(&self, id: ElementId) -> Option<CharacterBounds> {
        let el = self.elements.get(&id)?;
        let edit = el.edit.as_ref()?;
        let cl = el.content_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(id)?;
        let taffy_node = self.layout.projection.node_id(id)?;
        let box_layout = self.layout.projection.taffy.layout(taffy_node).ok()?;
        let content_x = ex + box_layout.border.left + box_layout.padding.left;
        let content_y = ey + box_layout.border.top + box_layout.padding.top;
        use parley::{Affinity, Cursor};
        let cursor = Cursor::from_byte_index(
            &cl.layout,
            edit.cursor_byte_index,
            Affinity::Upstream,
        );
        let bbox = cursor.geometry(&cl.layout, 1.5_f32);
        Some(CharacterBounds {
            x: content_x + bbox.x0 as f32,
            y: content_y + bbox.y0 as f32,
            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
            height: (bbox.y1 - bbox.y0) as f32,
        })
    }

    /// `id` の解決済み実効 visual（継承 + ビューポート variant + 擬似）。ADR-0067, ADR-0081。
    pub fn element_effective_visual(&self, id: ElementId) -> Option<Visual> {
        let el = self.elements.get(&id)?;
        let ctx = effective_visual::inherited_context_at(&self.elements, id);
        let interaction = self.interaction_snapshot();
        Some(effective_visual::resolve_effective(
            &ctx,
            &el.visual,
            &el.viewport_variants,
            self.viewport,
            &el.pseudo_styles,
            &interaction,
            id,
        ))
    }

    /// `now_ms` における `id` の表示 visual: 解決済みの実効ターゲット（ADR-0067）に、
    /// 保持された進行中トランジション（ADR-0093）を `now_ms` まで補間したもの。
    /// 読み取り専用（`&self`）で、render 経路と同じブレンドをサンプルするが render の
    /// メモ化トランジション状態は進めない。これによりトランジション途中の値を
    /// `render()` → SceneGraph 走査なしに 1 クエリで観測できる。`id` が未知なら `None`。
    pub fn element_displayed_visual(&self, id: ElementId, now_ms: f64) -> Option<Visual> {
        let resolved = self.element_effective_visual(id)?;
        Some(match self.scene_lowering.anchors.get(&id) {
            Some(entry) => entry.sample_displayed(&resolved, now_ms),
            None => resolved,
        })
    }

    /// (x, y) を境界矩形に含む最深の要素を返す。どれにも当たらなければ None。
    /// 直近 render パスのレイアウトを使う。
    pub fn hit_test(&self, x: f32, y: f32) -> Option<ElementId> {
        let root = self.root?;
        // `hit_test_walk` は照会点を各スクロールビューのコンテンツ空間へ降ろすので、
        // `(hx, hy)` は `box_hit` 自身のレイアウト座標 — そのジオメトリとテキスト
        // レイアウトが存在する空間 — における点になる。
        let (box_hit, hx, hy) = hit_test_walk(self, root, x, y)?;
        inline_text::resolve_ifc_inline_hit(self, box_hit, hx, hy)
    }

    /// レイアウトを実行し、全要素を絶対位置と visual 状態とともに返す。安定 ElementId
    /// をキーにするので、フレームをまたいだ DOM ノードのマッピングキーに使える。
    pub fn resolved_elements(&mut self) -> Vec<(ElementId, ResolvedElement)> {
        self.commit_frame();
        let interaction = self.interaction_snapshot();
        let mut out = Vec::new();
        if let Some(root) = self.root {
            walk_resolved(
                &self.elements,
                &self.layout.projection,
                root,
                0.0,
                0.0,
                effective_visual::InheritedVisualContext::root(),
                &interaction,
                self.viewport,
                &mut out,
            );
        }
        out
    }

    // ── internals ────────────────────────────────────────────────────────

    fn detach_from_current_parent(&mut self, child: ElementId) {
        let parent = match self.elements.get(&child).and_then(|c| c.parent) {
            Some(p) => p,
            None => return,
        };
        self.elements
            .get_mut(&parent)
            .unwrap()
            .children
            .retain(|&c| c != child);
        self.elements.get_mut(&child).unwrap().parent = None;
        self.mark_child_detachment_dirty(parent, child);
    }

    pub(crate) fn mark_pseudo_activation_dirty(&mut self, id: ElementId, state: PseudoState) {
        let props = match self.elements.get(&id) {
            Some(el) => el.pseudo_styles.props(state),
            None => return,
        };
        if props.is_empty() {
            return;
        }
        let reach = self.classify_style_props(id, props).reach;
        let affects_shaping = pseudo_state::pseudo_affects_text_shaping(props);
        // 擬似ブロックは box の visual とテキストスタイルの両方を持ちうるので、要素は
        // 常に visual-dirty、加えてテキストシェイピングが影響を受けるときは shape-dirty。
        // どちらも単一のルーティング seam を通す。
        self.apply_change_at(
            id,
            Change {
                dirty_kind: DirtyKind::Visual,
                reach,
            },
        );
        if affects_shaping {
            self.apply_change_at(
                id,
                Change {
                    dirty_kind: DirtyKind::Shape,
                    reach,
                },
            );
        }
        // トランジションのトリガはここではなく `resolve_effective` の lowering seam に
        // ある（ADR-0093）。要素を visual-dirty にすれば再 lowering され、そこで
        // プロパティごとの差分が補間を開始する。
    }

    fn mark_text_content_dirty(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        self.apply_change_at(
            id,
            Change {
                dirty_kind: DirtyKind::Shape,
                reach,
            },
        );
    }

    fn mark_child_attachment_dirty(&mut self, parent: ElementId, child: ElementId) {
        let parent_ctx = self.element_context(parent);
        let child_ctx = self.element_context(child);
        let change = visual_invalidation::classify_attachment(parent_ctx, child_ctx);
        // 接続の両端点は同じ `Change` を報告する。shape 接続は両者を親 IFC root へ
        // ルートする（冪等）。structure 接続は親と子を独立に仕込む。いずれにせよ
        // dirty 集合の結合はここではなく `route_change` にある。
        self.apply_change_at(parent, change);
        self.apply_change_at(child, change);
    }

    /// 無効化分類器のために要素のトポロジカルなコンテキストを構築する。稼働中の
    /// ツリーを読む。分類器自体は純粋なまま。
    pub(crate) fn element_context(&self, id: ElementId) -> ElementContext {
        visual_invalidation::element_context_in(&self.elements, id)
    }

    fn mark_child_detachment_dirty(&mut self, parent: ElementId, child: ElementId) {
        self.mark_child_attachment_dirty(parent, child);
    }

}

impl Default for ElementTree {
    fn default() -> Self {
        Self::new()
    }
}

fn walk_resolved(
    elements: &HashMap<ElementId, Element>,
    projection: &TaffyProjection,
    id: ElementId,
    ox: f32,
    oy: f32,
    inherited: effective_visual::InheritedVisualContext,
    interaction: &InteractionSnapshot,
    viewport: (f32, f32),
    out: &mut Vec<(ElementId, ResolvedElement)>,
) {
    let (taffy_node, el) = match projection.traversal_step(elements, id) {
        Some(TraversalStep::Visit(taffy_node, el)) => (Some(taffy_node), el),
        Some(TraversalStep::Skip(el)) => (None, el),
        None => return,
    };
    let inherited_base = effective_visual::apply_text_inheritance(&inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let taffy_node = match taffy_node {
        Some(n) => n,
        None => {
            for &child in &el.children {
                walk_resolved(
                    elements,
                    projection,
                    child,
                    ox,
                    oy,
                    child_inherited.clone(),
                    interaction,
                    viewport,
                    out,
                );
            }
            return;
        }
    };
    let layout = match projection.taffy.layout(taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ox + layout.location.x;
    let y = oy + layout.location.y;
    let visual = effective_visual::resolve_effective(
        &inherited,
        &el.visual,
        &el.viewport_variants,
        viewport,
        &el.pseudo_styles,
        interaction,
        id,
    );

    let display_text_content = if el.kind == ElementKind::TextInput {
        el.edit.as_ref().map(|edit| edit.display_text())
    } else {
        None
    };

    out.push((
        id,
        ResolvedElement {
            kind: el.kind,
            x,
            y,
            width: layout.size.width,
            height: layout.size.height,
            background_color: visual.background_color,
            opacity: visual.opacity,
            border_radius: visual.border_radius,
            border_width: visual.border_width,
            border_color: visual.border_color,
            text_color: visual.text_color,
            font_size: visual.font_size,
            font_weight: visual.font_weight,
            z_index: visual.z_index,
            text: el.text.clone(),
            src: el.src.clone(),
            text_content: display_text_content,
            font_family: visual.font_family.clone(),
            aria_label: el.aria_label.clone(),
            role: el.role.clone(),
        },
    ));

    for &child in &el.children {
        walk_resolved(
            elements,
            projection,
            child,
            x,
            y,
            child_inherited.clone(),
            interaction,
            viewport,
            out,
        );
    }
}

/// 最深ヒット要素を、照会点を*その要素の*レイアウト座標空間で表したものとともに
/// 返す。すなわち、降りてきた各 ScrollView の `scroll_offset` を累積してずらした
/// 画面上の点。ローカル点が要る呼び出し側（インラインテキスト解決）はタプルから読む。
fn hit_test_walk(tree: &ElementTree, id: ElementId, x: f32, y: f32) -> Option<(ElementId, f32, f32)> {
    let (ex, ey, ew, eh) = tree.layout.geometry(id)?;
    if x < ex || y < ey || x >= ex + ew || y >= ey + eh {
        return None;
    }
    tree.elements.get(&id)?;
    // スクロールビューはコンテンツを −scroll_offset だけ平行移動して描く
    // （`scene_build` の Clip 下の Group）ので、子孫を判定する前に照会点を
    // +scroll_offset ずらし、実際に描かれた位置に合わせる。非スクローラはオフセット
    // (0,0) を持ち、ネストしたスクローラは再帰がずらした点を降ろすことで合成される。
    // これがないとヒットテストは未スクロールのレイアウトを読み、スクロールで入った子
    // （例: ネストしたスクロール箱）を取り逃し、ホイールが誤った ScrollView へチェーン
    // してしまう（二重スクロール）。
    let (sx, sy) = tree.element_get_scroll_offset(id);
    let (cx, cy) = (x + sx, y + sy);
    // 子は paint の逆順（`.rev()`）で訪れ、最前面の要素が勝つようにする。
    // `ordered_children` を共有することでヒットテストを paint の正確な逆順に保つ。
    for child in tree.ordered_children(id).into_iter().rev() {
        if let Some(hit) = hit_test_walk(tree, child, cx, cy) {
            return Some(hit);
        }
    }
    if tree.elements.get(&id).is_some_and(|e| e.disabled) {
        return None;
    }
    Some((id, x, y))
}

/// ルーティング seam の背後の稼働中 dirty 集合: `ElementEngine` の visual / shape /
/// structure 集合と `TaffyProjection` のジオメトリ集合（ADR-0099）。`route_change` が
/// これを駆動し、engine と projection を一緒にマークする。
struct EngineProjectionSink<'a> {
    engine: &'a mut ElementEngine,
    projection: &'a mut crate::element::taffy_projection::TaffyProjection,
}

impl DirtySink for EngineProjectionSink<'_> {
    fn mark_visual(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        self.engine.mark_visual_dirty(id, reach);
    }
    fn mark_shape(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        self.engine.mark_shape_dirty(id, reach);
    }
    fn mark_structure(&mut self, id: ElementId) {
        self.engine.mark_structure_dirty(id);
    }
    fn mark_geometry(&mut self, id: ElementId) {
        self.projection.mark_dirty(id);
    }
}

pub(crate) fn apply_visual(visual: &mut Visual, prop: &StyleProp, text_dirty: &mut bool) {
    match prop {
        StyleProp::BackgroundColor(c) => visual.background_color = Some(*c),
        StyleProp::Opacity(v) => visual.opacity = v.clamp(0.0, 1.0),
        StyleProp::BorderRadius(v) => visual.border_radius = v.max(0.0),
        StyleProp::BorderWidth(v) => visual.border_width = v.max(0.0),
        StyleProp::BorderColor(c) => visual.border_color = Some(*c),
        StyleProp::BorderStyle(v) => visual.border_style = *v,
        StyleProp::BoxShadow(shadows) => visual.box_shadow = shadows.clone(),
        StyleProp::Overflow(v) => visual.overflow = *v,
        StyleProp::MaxLines(v) => {
            visual.max_lines = if *v == 0 { None } else { Some(*v) };
            *text_dirty = true;
        }
        StyleProp::TextOverflow(v) => {
            visual.text_overflow = *v;
            *text_dirty = true;
        }
        StyleProp::FontSize(v) => {
            visual.font_size = Some(v.max(0.0));
            *text_dirty = true;
        }
        StyleProp::FontFamily(f) => {
            visual.font_family = if f.is_empty() { None } else { Some(f.clone()) };
            *text_dirty = true;
        }
        StyleProp::FontWeight(v) => {
            visual.font_weight = Some(v.clamp(1.0, 1000.0));
            *text_dirty = true;
        }
        StyleProp::Color(c) => {
            visual.text_color = Some(*c);
            *text_dirty = true;
        }
        StyleProp::FontStyle(v) => {
            visual.font_style = Some(*v);
            *text_dirty = true;
        }
        StyleProp::TextDecoration(v) => {
            visual.text_decoration = Some(*v);
            *text_dirty = true;
        }
        StyleProp::Cursor(v) => visual.cursor = Some(*v),
        StyleProp::DefaultColor(c) => visual.default_color = Some(*c),
        StyleProp::DefaultFontSize(v) => visual.default_font_size = Some(v.max(0.0)),
        StyleProp::DefaultFontWeight(v) => {
            visual.default_font_weight = Some(v.clamp(1.0, 1000.0));
        }
        StyleProp::DefaultFontFamily(f) => {
            visual.default_font_family = if f.is_empty() {
                None
            } else {
                Some(f.clone())
            };
        }
        StyleProp::ZIndex(z) => visual.z_index = *z,
        StyleProp::TransitionDuration(v) => visual.transition_duration = v.max(0.0),
        StyleProp::TransitionTiming(v) => visual.transition_timing = *v,
        _ => {}
    }
}

pub(super) fn next_ancestor_scroll_view(tree: &ElementTree, after: ElementId) -> Option<ElementId> {
    let mut id = tree.element_parent(after)?;
    loop {
        if tree.element_kind(id) == Some(ElementKind::ScrollView) {
            return Some(id);
        }
        id = tree.element_parent(id)?;
    }
}

#[doc(hidden)]
impl ElementTree {
    pub fn test_scene_lowering_built(&self) -> bool {
        self.scene_lowering.built
    }

    pub fn test_scene_lowering_walk_count(&self) -> usize {
        self.scene_lowering.walk_count
    }

    pub fn test_visual_dirty_contains(&self, id: ElementId) -> bool {
        self.engine.visual_dirty.contains_key(&id)
    }

    /// 直近の `render()` 後に、まだ再描画を要するフレームが残っているか。`render()` は
    /// `visual_dirty` を drain した後、**まだ補間中の transition を持つ要素だけ**を
    /// 再マークする（line 1505 付近・ADR-0086/0093）。したがって render 後にここが
    /// `true` なのは「進行中 transition がある」ときで、App Host はこの間 `request_redraw`
    /// を出し続け、空になればフレームループを idle に落とす（ADR-0117）。
    ///
    /// 注意：カーソル点滅は render 冒頭でマークされ同フレームで drain されるため、
    /// またリリース済みスクロール物理は現状 web adapter 側にあるため、どちらも render 後の
    /// `visual_dirty` には現れない。これらの継続要求を畳み込むのは後続作業。
    pub fn has_pending_visual_work(&self) -> bool {
        !self.engine.visual_dirty.is_empty()
    }

    pub fn test_shape_dirty_contains(&self, id: ElementId) -> bool {
        self.engine.shape_dirty.contains(&id)
    }

    /// `id` で連続プロパティのトランジションが現在進行中かどうか。状態は保持された
    /// lowering にあるので、直近の `render()` パスを反映する。
    pub fn test_transition_active(&self, id: ElementId) -> bool {
        self.scene_lowering
            .anchors
            .get(&id)
            .is_some_and(|entry| entry.transitions.is_active())
    }

    /// 要素のシェイプ済みテキストでレイアウトされた行数。
    pub fn test_text_line_count(&self, id: ElementId) -> Option<usize> {
        self.elements
            .get(&id)
            .and_then(|el| el.text_layout.as_ref())
            .map(|tl| tl.layout.lines().count())
    }

    /// 要素の IFC レイアウトの、切り詰め後のシェイプ済みテキスト。
    pub fn test_shaped_text(&self, id: ElementId) -> Option<String> {
        self.elements
            .get(&id)
            .and_then(|el| el.text_layout.as_ref())
            .map(|tl| tl.text.to_string())
    }

    /// テストシーム（ADR-0042）: フォントコレクションを WASM ランタイムに合わせて
    /// 再構成する — システムフォントなし、`default_font` を既定 family にする。
    /// ホストにインストールされたフォントに依存せず、core テストで実際の
    /// `.notdef → FetchFont → register_font` リトライ経路を駆動する。
    pub fn test_set_wasm_like_fonts(&mut self, default_font: Vec<u8>) {
        self.layout.set_wasm_like_font_context(default_font);
        self.engine.mark_fonts_dirty();
    }

    /// テストヘルパー: 要素のテキストレイアウトのシェイプ済みグリフ id。`.notdef`
    /// （tofu）はグリフ id `0` なので、0 を含まないレイアウトはテキストに実グリフが
    /// 描かれた証拠になる。
    pub fn test_element_glyph_ids(&self, id: ElementId) -> Vec<u32> {
        self.elements
            .get(&id)
            .and_then(|el| el.text_layout.as_ref())
            .map(|tl| {
                tl.runs
                    .iter()
                    .flat_map(|run| run.glyphs.iter().map(|g| g.id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// dirty 集合を drain せずに `render()` のカーソル点滅刻みをミラーする。
    pub fn test_tick_cursor_blink(&mut self, timestamp_ms: f64) -> bool {
        let Some(id) = self.layout.advance_cursor_blink(
            &mut self.elements,
            self.interaction.focused_element,
            timestamp_ms,
        ) else {
            return false;
        };
        self.engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        true
    }

    pub fn test_element_anchor_id(&self, id: ElementId) -> crate::node::NodeId {
        self.scene_lowering
            .anchors
            .get(&id)
            .expect("element anchor")
            .anchor_id
    }

    pub fn test_scene_full_rebuild_draw_ops(&self) -> Vec<crate::render::DrawOp> {
        use crate::render::{render_scene_graph, RecordingPainter};
        let sg = scene_build::build_ephemeral(self);
        let mut painter = RecordingPainter::new();
        render_scene_graph(&sg, &mut painter);
        painter.into_ops()
    }
}

#[cfg(test)]
mod value_guard_tests {
    use super::ElementTree;
    use crate::element::kind::ElementKind;

    /// programmatic value set（ADR-0007）は、組成中でなく差分があるときだけ適用する。
    #[test]
    fn set_text_content_if_idle_applies_diff_when_not_composing() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        assert!(tree.element_set_text_content_if_idle(input, "abc"));
        assert_eq!(tree.element_get_text_content(input), "abc");
        // 差分なし → no-op（キーストローク echo の抑止）。
        assert!(!tree.element_set_text_content_if_idle(input, "abc"));
    }

    /// IME 組成中（preedit あり）は書き戻さない（preedit / cursor を壊さない）。
    #[test]
    fn set_text_content_if_idle_is_noop_while_composing() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.element_set_text_content_if_idle(input, "ab");
        tree.element_set_preedit(input, "へん");
        assert!(!tree.element_set_text_content_if_idle(input, "xyz"));
        // 確定済み text_content は不変（display は content + preedit）。
        assert_eq!(tree.element_get_text_content(input), "abへん");
    }

    /// edit を持たない非 input 要素には適用されない。
    #[test]
    fn set_text_content_if_idle_ignores_non_input_elements() {
        let mut tree = ElementTree::new();
        let view = tree.element_create(1, ElementKind::View);
        assert!(!tree.element_set_text_content_if_idle(view, "x"));
    }
}

