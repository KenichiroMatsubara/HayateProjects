//! Canvas Mode レンダラ（`HayateElementRenderer`）。ADR-0077 参照。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::pointer_input::{self, PointerInput, PointerInputGuard};
use crate::resize_observer::{self, ResizeObserverGuard};
use crate::scroll_drag::{self, MoveOutcome, ScrollGesture, ScrollPhysicsTuning};

use hayate_core::{
    BorderStyleValue, Color, CursorValue, DocumentEventKind, EditIntent, ElementId, ElementKind,
    ElementTree, Event, FontStyleValue, RenderImage, RenderImageAlphaType, RenderImageFormat,
    StyleProp, StylePropKind, TextDecorationValue,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlCanvasElement;

use crate::apply_mutations_dispatch::{
    apply_mutations_batch, unset_kind_from_u32, ApplyMutationsHost,
};
use crate::backend::{CanvasBackend, SelectedBackend};
use crate::builtin_fonts::font_url_for_renderer;
use crate::generated::encode_deliveries;
use crate::ime_bridge::WebImeBridge;
use crate::style_packet;

use crate::shared::{element_id_from_f64, element_id_to_f64, fetch_bytes, kind_from_u32};

/// アダプタが非同期取得したフォント。次の `poll_events()` でツリーへ流し込む
/// （単一スレッド WASM なので Rc<RefCell> で安全）。
type FontQueue = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// 取得に失敗したファミリ。次の `render()` で `tree.font_fetch_failed` へ流し込み、
/// core が再要求（または断念）できるようにする。これがないとファミリが `pending`
/// に固定されたまま残る。
type FontFailureQueue = Rc<RefCell<Vec<String>>>;

/// ファミリ別の失敗回数。指数バックオフでリトライ間隔を空けるためだけに使う。
/// 断念の判断（予算）は core が持ち、ここはタイミングだけを持つ。
type FontFetchAttempts = Rc<RefCell<HashMap<String, u32>>>;
/// 帯域外で解決される非同期クリップボード読み取り（Ctrl/Cmd+V）。各 `readText()` が
/// `(target, text)` をここへ積み、次の `render()` 冒頭で `element_paste` 経由で
/// core へ流し込む（ADR-0097）。
type PendingPaste = Rc<RefCell<Vec<(ElementId, String)>>>;

/// 失敗報告前のバックオフ: `BASE << (attempt - 1)`（上限あり）。
/// デプロイ直後は jsdelivr が一時的な 403/429 を返すことがあるため、最初のリトライは
/// 早く、以降は徐々に間隔を空ける。
const FETCH_BACKOFF_BASE_MS: i32 = 400;
const FETCH_BACKOFF_MAX_MS: i32 = 5_000;

/// `setTimeout` で `ms` 後に解決する。fetch future が失敗報告前にバックオフ遅延を
/// await できるようにする。
async fn backoff_sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        if let Some(win) = web_sys::window() {
            let cb = Closure::once_into_js(move || {
                let _ = resolve.call0(&JsValue::NULL);
            });
            let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.unchecked_ref(),
                ms,
            );
        }
    });
    let _ = JsFuture::from(promise).await;
}

/// core の `Clipboard` シームの Web 実装（ADR-0097）。コピー（Cmd/Ctrl+C）は core で
/// 走り、選択テキストがここへ渡され、アダプタが非同期 Clipboard API で書き込む。書き込みは
/// fire-and-forget で、core が処理したユーザージェスチャの keydown 内で同期的に開始する。
/// これがブラウザの書き込み許可要件を満たす。
struct WebClipboard;

impl hayate_core::Clipboard for WebClipboard {
    fn write_text(&self, text: &str) {
        if let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard()) {
            let _ = clipboard.write_text(text);
        }
    }
}

// ── Canvas Mode レンダラ ─────────────────────────────────────────────────

#[wasm_bindgen]
pub struct HayateElementRenderer {
    canvas: HtmlCanvasElement,
    backend: SelectedBackend,
    tree: ElementTree,
    /// wgpu サーフェスのクリア色。WIT の `render` 署名がこれを持たなくなったため
    /// `render(timestamp_ms)` から分離されている（ADR-0032 で render は timestamp のみ）。
    /// `set_background_color` で別途設定する。
    background: [f32; 4],
    /// future が取得したフォント。次の poll_events でツリーへ適用する。
    font_queue: FontQueue,
    /// 取得に失敗したファミリ。次の `render()` で core へ報告する。
    font_failure_queue: FontFailureQueue,
    /// ファミリ別の失敗回数。指数リトライバックオフ用。
    font_fetch_attempts: FontFetchAttempts,
    /// 毎 render で同期する IME 候補ウィンドウの境界（ADR-0069）。
    ime: WebImeBridge,
    /// ResizeObserver コールバックが次の `render()` 用にビューポート計測値をキューする。
    pending_resize: Rc<RefCell<Option<resize_observer::CanvasResizeMetrics>>>,
    last_viewport: Rc<RefCell<(f32, f32)>>,
    _resize_observer: ResizeObserverGuard,
    /// 自前配線のポインタリスナ（ADR-0080）がここへ積む。`render()` 冒頭で到着順に排出。
    pending_pointer: Rc<RefCell<Vec<PointerInput>>>,
    /// 排出時に適用した直近の move 位置。フレーム境界をまたぐ 1px move コアレッシングの
    /// シードに使う。
    last_pointer_move: Option<(f32, f32)>,
    /// アクティブな touch/pen のドラッグ→スクロールジェスチャ。フレーム間で 1 つの
    /// scroll-view にロックされる（ADR-0082）。タッチ押下がない、または非スクロール領域
    /// への押下のときは `None`。
    scroll_gesture: Option<ScrollGesture>,
    /// スクロール中に記録する指のサンプル `(x, y, frame_ms)`。リリース時に
    /// `estimate_release_velocity` へ渡し慣性を起動する（ADR-0082 Amendment）。新規押下ごとにクリア。
    scroll_samples: Vec<(f32, f32, f64)>,
    /// アクティブドラッグの生（抵抗なし）の累積指オフセット。ラバーバンドの駆動に使う。
    /// 指はこれを 1:1 で動かし、表示オフセットは `rubber_band_offset(raw, …)` なので、
    /// 端を越えたオーバースクロールでは指に遅れる。スクロール中でないときは `None`。
    /// 最初の `Scroll` でシードし、押下/リリース/キャンセルでクリア。
    drag_raw: Option<(ElementId, (f32, f32))>,
    /// 進行中のリリース済みスクロール: ロックした scroll-view と、その軸別オフセット速度
    /// （px/ms）。毎フレーム `scroll_motion_step` が積分する（範囲内は摩擦、オーバースクロール中は
    /// バネ）。これによりフリックが惰性で滑り、端でバウンスし、定位置へ戻る。アニメーションが
    /// なければ `None`。
    scroll_motion: Option<(ElementId, (f32, f32))>,
    /// 直前の `render()` フレームのタイムスタンプ。慣性積分器が進めるフレーム間 `dt` 用。
    /// 最初のフレーム前は `None`。
    last_frame_ms: Option<f64>,
    /// スクロール物理の調整値。既定は正準の const。dev ビルドでは
    /// [`set_tuning`](Self::set_tuning) で `tuning.json` を上書きし、再ビルドなしに実機で
    /// 感触を調整できる。
    scroll_tuning: ScrollPhysicsTuning,
    /// 保留中の Ctrl/Cmd+V のため非同期クリップボードから読んだテキスト。次の `render()` で
    /// 適用する（ブラウザのクリップボード読み取りは非同期で、同期の
    /// `Clipboard::read_text` シームでは扱えない。ADR-0097）。
    pending_paste: PendingPaste,
    _pointer_input: PointerInputGuard,
}

#[wasm_bindgen]
impl HayateElementRenderer {
    pub async fn init(canvas: HtmlCanvasElement) -> Result<HayateElementRenderer, JsValue> {
        let rect = canvas.get_bounding_client_rect();
        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);
        let metrics =
            resize_observer::canvas_resize_metrics(rect.width() as f32, rect.height() as f32, dpr);
        canvas.set_width(metrics.buffer_width);
        canvas.set_height(metrics.buffer_height);

        let mut backend = SelectedBackend::init(canvas.clone()).await?;
        backend.resize(
            metrics.buffer_width,
            metrics.buffer_height,
            metrics.content_scale,
        );
        let mut tree = ElementTree::new();
        tree.set_viewport(metrics.viewport_width, metrics.viewport_height);
        // core のコピー（Cmd/Ctrl+C）がブラウザ Clipboard API に届くよう、Platform Adapter
        // のクリップボードを配線する（ADR-0097）。
        tree.set_clipboard(Box::new(WebClipboard));

        let pending_resize = Rc::new(RefCell::new(None));
        let last_viewport = Rc::new(RefCell::new((
            metrics.viewport_width,
            metrics.viewport_height,
        )));
        let resize_guard = resize_observer::attach_resize_observer(
            &canvas,
            pending_resize.clone(),
            last_viewport.clone(),
        )?;

        let pending_pointer = Rc::new(RefCell::new(Vec::new()));
        let pointer_guard =
            pointer_input::attach_pointer_input(&canvas, pending_pointer.clone())?;

        Ok(Self {
            canvas,
            backend,
            tree,
            background: [0.0, 0.0, 0.0, 1.0],
            font_queue: Rc::new(RefCell::new(Vec::new())),
            font_failure_queue: Rc::new(RefCell::new(Vec::new())),
            font_fetch_attempts: Rc::new(RefCell::new(HashMap::new())),
            ime: WebImeBridge::default(),
            pending_resize,
            last_viewport,
            _resize_observer: resize_guard,
            pending_pointer,
            last_pointer_move: None,
            scroll_gesture: None,
            scroll_samples: Vec::new(),
            drag_raw: None,
            scroll_motion: None,
            last_frame_ms: None,
            scroll_tuning: ScrollPhysicsTuning::default(),
            pending_paste: Rc::new(RefCell::new(Vec::new())),
            _pointer_input: pointer_guard,
        })
    }

    /// 以降の各 `render()` で使う wgpu サーフェスのクリア色を設定する。WIT には含まれず、
    /// timestamp のみの `render`（ADR-0032）を補完する。デモが毎フレーム色を再発行せずに
    /// カラーピッカーを駆動できる。
    pub fn set_background_color(&mut self, r: f64, g: f64, b: f64) {
        self.background = [r as f32, g as f32, b as f32, 1.0];
    }

    /// dev 専用の `tuning.json` による味付け定数の上書きを適用する。`json` は生のファイル
    /// テキスト。ホストは `tuning.json` を取得し、`init` 後・最初のフレーム前に一度だけ呼ぶ。
    /// ファイルが無ければホストは呼ばない（既定値のまま）。不正な JSON や未知のキーは `Err` を
    /// 返し、ホストが握り潰してコンパイル時の既定値を維持する。ファイルを編集して F5 リロード
    /// すれば再ビルドなしで再適用される。
    pub fn set_tuning(&mut self, json: JsValue) -> Result<(), JsValue> {
        let text = json
            .as_string()
            .ok_or_else(|| JsValue::from_str("set_tuning: expected a JSON string"))?;
        let parsed = crate::tuning::TuningJson::parse(&text)
            .map_err(|e| JsValue::from_str(&format!("set_tuning: {e}")))?;
        self.scroll_tuning = parsed.scroll_tuning();
        self.tree.set_chrome_tuning(parsed.chrome_tuning());
        Ok(())
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
    }

    /// 呼び出し側が指定した ID で要素を登録する。JS が単調増加カウンタで ID を生成するため、
    /// ID 割り当てのための WASM 往復が不要になる。
    pub fn element_create(&mut self, id: f64, kind: u32) -> Result<(), JsValue> {
        let k = kind_from_u32(kind)?;
        self.tree.element_create(id as u64, k);
        Ok(())
    }

    pub fn element_set_text(&mut self, id: f64, text: &str) {
        self.tree.element_set_text(element_id_from_f64(id), text);
    }

    pub fn element_set_src(&mut self, id: f64, url: &str) {
        self.tree.element_set_src(element_id_from_f64(id), url);
    }

    pub fn element_set_disabled(&mut self, id: f64, disabled: bool) {
        self.tree
            .element_set_disabled(element_id_from_f64(id), disabled);
    }

    pub fn element_set_selectable(&mut self, id: f64, selectable: bool) {
        self.tree
            .element_set_selectable(element_id_from_f64(id), selectable);
    }

    pub fn element_set_multiline(&mut self, id: f64, multiline: bool) {
        self.tree
            .element_set_multiline(element_id_from_f64(id), multiline);
    }

    pub fn element_set_style(&mut self, id: f64, packed: &[f32]) -> Result<(), JsValue> {
        let props = style_packet::decode(packed)?;
        self.tree.element_set_style(element_id_from_f64(id), &props);
        Ok(())
    }

    /// Hayate CSS の擬似クラスブロック（`:hover` / `:active` / `:focus`）。
    pub fn element_set_pseudo_style(
        &mut self,
        id: f64,
        state: u32,
        packed: &[f32],
    ) -> Result<(), JsValue> {
        let pseudo = hayate_core::PseudoState::from_u32(state)
            .ok_or_else(|| JsValue::from_str(&format!("unknown pseudo-state {state}")))?;
        let props = style_packet::decode(packed)?;
        self.tree
            .element_set_pseudo_style(element_id_from_f64(id), pseudo, &props);
        Ok(())
    }

    /// レイアウトの上に 2D アフィン変換を適用する。引数は WIT の `affine` レコードフィールド
    /// に対応する（列優先: xx,yx,xy,yy,dx,dy）。恒等変換 (1,0,0,1,0,0) を渡すと以前の変換を
    /// 打ち消す。
    pub fn element_set_transform(
        &mut self,
        id: f64,
        xx: f64,
        yx: f64,
        xy: f64,
        yy: f64,
        dx: f64,
        dy: f64,
    ) {
        self.tree
            .element_set_transform(element_id_from_f64(id), Some([xx, yx, xy, yy, dx, dy]));
    }

    pub fn element_append_child(&mut self, parent: f64, child: f64) {
        self.tree
            .element_append_child(element_id_from_f64(parent), element_id_from_f64(child));
    }

    pub fn element_insert_before(&mut self, parent: f64, child: f64, before: f64) {
        self.tree.element_insert_before(
            element_id_from_f64(parent),
            element_id_from_f64(child),
            element_id_from_f64(before),
        );
    }

    pub fn element_remove(&mut self, id: f64) {
        let eid = element_id_from_f64(id);
        self.remove_subtree(eid);
    }

    fn remove_subtree(&mut self, id: ElementId) {
        self.tree.element_remove(id);
    }

    /// 要素の現在のテキストを返す。Canvas Mode は `element_set_text` を即時適用するため
    /// （ADR-0037）、直近のセッタ呼び出しがそのまま反映される。
    pub fn element_get_text(&self, id: f64) -> String {
        self.tree.element_get_text(element_id_from_f64(id))
    }

    /// 直近のレイアウトパスでの要素の絶対境界 [x, y, width, height] を返す。id が未知、または
    /// まだレイアウトされていない場合はゼロ。WIT 準拠（`element-get-bounds`）。
    pub fn element_get_bounds(&self, id: f64) -> Box<[f32]> {
        let eid = element_id_from_f64(id);
        let (x, y, w, h) = self
            .tree
            .element_layout_rect(eid)
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        vec![x, y, w, h].into_boxed_slice()
    }

    /// 継承＋擬似状態を解決した後の `id` のスタイル（ADR-0067）。`id` が未知なら `null`、
    /// それ以外は実効 `Visual` フィールドを持つ JS オブジェクト（camelCase キー、色は
    /// `{r,g,b,a}`）。
    pub fn element_effective_visual(&self, id: f64) -> JsValue {
        let eid = element_id_from_f64(id);
        let Some(visual) = self.tree.element_effective_visual(eid) else {
            return JsValue::NULL;
        };

        let obj = js_sys::Object::new();
        let set = |key: &str, value: JsValue| {
            js_sys::Reflect::set(&obj, &JsValue::from_str(key), &value).unwrap();
        };
        set("backgroundColor", color_to_js(visual.background_color));
        set("opacity", JsValue::from_f64(visual.opacity as f64));
        set("borderRadius", JsValue::from_f64(visual.border_radius as f64));
        set("borderWidth", JsValue::from_f64(visual.border_width as f64));
        set("borderColor", color_to_js(visual.border_color));
        set("borderStyle", border_style_to_js(visual.border_style));
        set("textColor", color_to_js(visual.text_color));
        set(
            "fontSize",
            visual
                .font_size
                .map(|v| JsValue::from_f64(v as f64))
                .unwrap_or(JsValue::NULL),
        );
        set(
            "fontWeight",
            visual
                .font_weight
                .map(|v| JsValue::from_f64(v as f64))
                .unwrap_or(JsValue::NULL),
        );
        set("fontStyle", font_style_to_js(visual.font_style));
        set("textDecoration", text_decoration_to_js(visual.text_decoration));
        set("zIndex", JsValue::from_f64(visual.z_index as f64));
        set(
            "fontFamily",
            visual
                .font_family
                .map(|f| JsValue::from_str(&f))
                .unwrap_or(JsValue::NULL),
        );
        obj.into()
    }

    pub fn set_root(&mut self, id: f64) {
        self.tree.set_root(element_id_from_f64(id));
    }

    /// カーソル点滅を進め、レイアウトを実行し、提示する。`timestamp_ms` は単調増加クロック
    /// （例: `performance.now()`）であること。ミューテーションは `element_*` セッタが即時適用
    /// するため（ADR-0037）、`render` はレイアウトのみを駆動する。
    pub fn render(&mut self, timestamp_ms: f64) -> Result<(), JsValue> {
        let pending = self.pending_resize.borrow_mut().take();
        if let Some(metrics) = pending {
            self.apply_resize(metrics);
        }
        self.drain_pointer_inputs(timestamp_ms);
        // このフレームの入力を排出した後（新規押下はアニメーションを中断し、リリースは起動する）、
        // 進行中のスクロール運動をフレーム間 dt だけ進める。慣性・バウンス・スプリングバックが
        // レイアウトと同じ rAF ループで積分される。
        self.step_scroll_motion(timestamp_ms);
        self.last_frame_ms = Some(timestamp_ms);
        // 失敗した取得をまず core へ報告する。各報告がフォントを dirty にするので、下の
        // commit_layout が再シェイプし、欠落を再検出し、次の poll_events で FetchFont を
        // 再発行する。core が断念したファミリは再要求しなくなるので、そのバックオフ
        // カウンタも破棄する。
        let failures: Vec<String> = self.font_failure_queue.borrow_mut().drain(..).collect();
        for family in failures {
            if !self.tree.font_fetch_failed(&family) {
                self.font_fetch_attempts.borrow_mut().remove(&family);
            }
        }
        // 取得完了フォントを layout より前に登録することで、同フレーム内で
        // fonts_dirty → compute_layout → 正しいグリフ、が成立する
        // （poll_events より先に render が呼ばれる raf ループでも豆腐にならない）。
        let loaded: Vec<(String, Vec<u8>)> = self.font_queue.borrow_mut().drain(..).collect();
        for (family, bytes) in loaded {
            self.font_fetch_attempts.borrow_mut().remove(&family);
            self.tree.register_font(&family, bytes);
        }
        // 前フレーム以降に非同期 Ctrl/Cmd+V の読み取りが解決したクリップボードテキストを、
        // レイアウト前に適用する。貼り付けテキストがこのフレームでシェイプされる。
        let pasted: Vec<(ElementId, String)> = self.pending_paste.borrow_mut().drain(..).collect();
        for (id, text) in pasted {
            self.tree.element_paste(id, &text);
        }
        let sg = self.tree.render(timestamp_ms);
        let present = self.backend.render_scene(sg, self.background);
        // ソフトキーボードの表示可否と候補ウィンドウ境界は core が一括で決める（ADR-0069）。
        // ブリッジは JS ホストが適用するために記録するだけ。
        self.tree.drive_ime(&mut self.ime);
        present
    }

    /// `url` から画像（PNG / JPEG / WebP）を取得し Image 要素に紐付ける。element_set_src の
    /// 後に呼ぶこと。解決するまで要素は空白で描画される。
    pub async fn load_image(&mut self, id: f64, url: String) -> Result<(), JsValue> {
        let eid = element_id_from_f64(id);
        let image_data = fetch_image(&url).await?;
        self.tree.element_set_image(eid, Arc::new(image_data));
        Ok(())
    }

    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.tree.on_pointer_down(x, y);
    }

    pub fn on_pointer_up(&mut self, x: f32, y: f32) {
        self.tree.on_pointer_up(x, y);
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        let result = self.tree.on_pointer_move(x, y);
        apply_resolved_cursor(&self.canvas, result.resolved_cursor);
    }

    pub fn on_wheel(&mut self, x: f32, y: f32, delta_x: f32, delta_y: f32) {
        if let Some(target) = self.tree.hit_test(x, y) {
            self.tree.apply_wheel_delta(target, delta_x, delta_y);
            self.tree.on_wheel(target, delta_x, delta_y);
        }
    }

    /// `render()` 冒頭で自前配線のポインタバッファを排出し、各入力を到着順に 1px move
    /// コアレッシングしながらツリーへ適用する（ADR-0080）。
    fn drain_pointer_inputs(&mut self, now_ms: f64) {
        let buffered: Vec<PointerInput> = self.pending_pointer.borrow_mut().drain(..).collect();
        if buffered.is_empty() {
            return;
        }
        let inputs = pointer_input::coalesce_pointer_inputs(buffered, self.last_pointer_move);
        self.last_pointer_move = pointer_input::final_anchor(&inputs, self.last_pointer_move);
        for input in inputs {
            self.apply_pointer_input(input, now_ms);
        }
    }

    fn apply_pointer_input(&mut self, input: PointerInput, now_ms: f64) {
        match input {
            PointerInput::Down {
                x,
                y,
                modifiers,
                kind,
            } => {
                // タップでも `:active` が出るよう、まず常に押下を送る。デバイスも転送し
                // Core がインタラクション単位で保持する。scroll-view 上の touch/pen 押下は
                // ドラッグ→スクロールジェスチャをロックする。slop を越えなければリリースは
                // 通常のクリックとして解決される。
                self.tree.on_pointer_down_with_kind(x, y, modifiers, kind);
                self.scroll_gesture = None;
                // 新規押下は惰性中のフリックやスプリングバックを中断し、コンテンツを即座に
                // 掴めるようにする。静止状態から始める。
                self.scroll_motion = None;
                self.drag_raw = None;
                self.scroll_samples.clear();
                if scroll_drag::is_drag_scroll_pointer(kind) {
                    if let Some(sv) = self
                        .tree
                        .hit_test(x, y)
                        .and_then(|hit| self.nearest_scroll_view(hit))
                    {
                        self.scroll_gesture = Some(ScrollGesture::new(sv, (x, y)));
                    }
                }
            }
            PointerInput::Move { x, y, kind } => {
                if let Some(mut gesture) = self.scroll_gesture.take() {
                    match gesture.on_move((x, y), self.scroll_tuning.slop_px) {
                        // まだ保留中のタップ — 押下を生かしたままにする。
                        MoveOutcome::Pending => {}
                        // slop を越えた: 押下を解除してタッチをスクロールにし、リリースで
                        // クリックを発火させない。引き継ぎ位置から速度トラッカーをシードする。
                        MoveOutcome::StartScroll => {
                            self.tree.on_pointer_cancel();
                            self.scroll_samples.push((x, y, now_ms));
                        }
                        // ロックした scroll-view を指でドラッグし（範囲内は 1:1、端を越えると
                        // ラバーバンドで抵抗）、リリース時にフリックを推定できるようサンプルを記録する。
                        MoveOutcome::Scroll { dx, dy } => {
                            self.apply_drag_delta(gesture.scroll_view, dx, dy);
                            self.scroll_samples.push((x, y, now_ms));
                        }
                    }
                    self.scroll_gesture = Some(gesture);
                } else {
                    let result = self.tree.on_pointer_move_with_kind(x, y, kind);
                    apply_resolved_cursor(&self.canvas, result.resolved_cursor);
                }
            }
            PointerInput::Up { x, y, kind } => {
                // slop を越えなかったタッチはタップ → クリックを解決する。スクロールに
                // なったものは既に押下がキャンセル済みなので、up を握り潰してリリース運動を
                // 起動する（サンプルしたフリックの慣性、および/またはオーバースクロールで
                // 離した場合のスプリングバック）。
                match self.scroll_gesture.take() {
                    Some(gesture) if !gesture.is_tap() => {
                        self.launch_scroll_motion(gesture.scroll_view)
                    }
                    _ => self.tree.on_pointer_up_with_kind(x, y, kind),
                }
            }
            PointerInput::Leave => self.tree.on_pointer_leave(),
            PointerInput::Cancel => {
                self.scroll_gesture = None;
                self.drag_raw = None;
                self.scroll_samples.clear();
                self.tree.on_pointer_cancel();
            }
            PointerInput::Wheel {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                if let Some(target) = self.tree.hit_test(x, y) {
                    self.tree.apply_wheel_delta(target, delta_x, delta_y);
                    self.tree.on_wheel(target, delta_x, delta_y);
                }
            }
        }
    }

    /// `id` から最も近い ScrollView 祖先（自身を含む）まで遡る。タッチジェスチャがロック
    /// する要素。公開の kind/parent クエリで Core のホイール経路 `nearest_scroll_view` を
    /// 再現し、ジェスチャロックをアダプタ側に置く（ADR-0082）。
    fn nearest_scroll_view(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.tree.element_kind(id) == Some(ElementKind::ScrollView) {
                return Some(id);
            }
            id = self.tree.element_parent(id)?;
        }
    }

    /// `sv` の軸別スクロール境界 `(max_x, max_y, dim_x, dim_y)`。`max` はスクロール可能範囲
    /// （`content − viewport`、0 で下限）、`dim` はラバーバンドのオーバースクロールが漸近する
    /// ビューポート幅。
    fn scroll_bounds(&self, sv: ElementId) -> (f32, f32, f32, f32) {
        let (max_x, max_y) = self.tree.element_scroll_max_offset(sv);
        let (_, _, view_w, view_h) = self
            .tree
            .element_layout_rect(sv)
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        (max_x, max_y, view_w, view_h)
    }

    /// ロックした scroll-view のオフセットをクランプせず設定し（SCR-02）、実際に動いたときは
    /// `Event::Scroll` を発火してパララックスや遅延読み込みがタッチスクロールにも反応する
    /// ようにする（ADR-0082）。`[0, max]` 外のオフセットは意図的で、ラバーバンドのドラッグも
    /// スプリングバック/バウンスのアニメーションもオーバースクロール領域にある。オフセット適用と
    /// スクロール通知は別の呼び出し。
    fn commit_scroll_offset(&mut self, sv: ElementId, nx: f32, ny: f32) {
        let (ox, oy) = self.tree.element_get_scroll_offset(sv);
        let (dx, dy) = (nx - ox, ny - oy);
        if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
            self.tree.element_set_scroll_offset(sv, nx, ny);
            self.tree.on_wheel(sv, dx, dy);
        }
    }

    /// 指のドラッグ差分をラバーバンド経由でロックした scroll-view に適用する。指は生の
    /// オフセットを 1:1 で動かし、表示オフセットは `rubber_band_offset(raw, …)`。範囲内では
    /// 指に正確に追従し、端を越えると抵抗を増しながら遅れる。生のアキュムレータは最初の
    /// ドラッグフレームで現在のオフセットからシードする。
    fn apply_drag_delta(&mut self, sv: ElementId, dx: f32, dy: f32) {
        let (max_x, max_y, dim_x, dim_y) = self.scroll_bounds(sv);
        let (rx, ry) = match self.drag_raw {
            Some((s, raw)) if s == sv => raw,
            _ => self.tree.element_get_scroll_offset(sv),
        };
        let (rx, ry) = (rx + dx, ry + dy);
        self.drag_raw = Some((sv, (rx, ry)));
        // 実際にスクロールできる軸だけラバーバンドする。スクロール不可な軸（`max == 0`）は
        // 原点に固定する。実機のモバイルブラウザは、スクロールするものがない軸をラバーバンド
        // しない（縦のみのページは横にバウンスしない）一方、本当に横スクロール可能な
        // コンテナ（`max > 0`）はバウンスする。iOS と同じ軸別オーバースクロール。
        let nx = if max_x > 0.0 {
            scroll_drag::rubber_band_offset(rx, max_x, dim_x, &self.scroll_tuning)
        } else {
            0.0
        };
        let ny = if max_y > 0.0 {
            scroll_drag::rubber_band_offset(ry, max_y, dim_y, &self.scroll_tuning)
        } else {
            0.0
        };
        self.commit_scroll_offset(sv, nx, ny);
    }

    /// スクロールジェスチャのリリース時、ロックした scroll-view にリリース運動を渡す。
    /// 記録した指サンプルから推定したフリック速度。本当のフリックがあれば惰性で滑り、
    /// オーバースクロールで指を離した（速度 ≈ 0）場合も、端が必ず定位置へ戻るよう
    /// アニメーションする。範囲内で終わる遅いリリースは何もアニメーションしない。
    fn launch_scroll_motion(&mut self, sv: ElementId) {
        let (vx, vy) = scroll_drag::estimate_release_velocity(&self.scroll_samples, &self.scroll_tuning);
        self.scroll_samples.clear();
        self.drag_raw = None;
        let (max_x, max_y, _, _) = self.scroll_bounds(sv);
        // スクロール不可な軸（`max == 0`）はフリックもオーバースクロールもしない。リリース
        // 速度を捨て、範囲外チェックでも無視することで、ユーザがスクロールできない軸を
        // リリース運動がバウンスさせないようにする（上のドラッグ時の軸別ゲートと一致）。
        let vx = if max_x > 0.0 { vx } else { 0.0 };
        let vy = if max_y > 0.0 { vy } else { 0.0 };
        let (ox, oy) = self.tree.element_get_scroll_offset(sv);
        let out_of_bounds = (max_x > 0.0 && (ox < 0.0 || ox > max_x))
            || (max_y > 0.0 && (oy < 0.0 || oy > max_y));
        let has_fling = vx.abs() >= self.scroll_tuning.min_velocity
            || vy.abs() >= self.scroll_tuning.min_velocity;
        self.scroll_motion = (has_fling || out_of_bounds).then_some((sv, (vx, vy)));
    }

    /// リリース済みスクロールを 1 フレーム進める。`scroll_motion_step` が各軸を積分し
    /// （範囲内は摩擦、オーバースクロール中はバネ）、新しいオフセットをクランプせず
    /// コミットし（指ドラッグ同様 `Event::Scroll` を発火）、減衰した速度を引き継ぐ。
    /// アニメーションは両軸が静止し**かつ** `[0, max]` 内に戻ったときに終わる。慣性バウンスは
    /// バネが定位置へ戻すまでオーバースクロール域を走り続ける。
    fn step_scroll_motion(&mut self, now_ms: f64) {
        let Some((sv, (vx, vy))) = self.scroll_motion else {
            return;
        };
        // 積分には実際のフレーム間スパンが必要。確保できるまで速度を持ち越す。
        let dt = match self.last_frame_ms {
            Some(prev) if now_ms > prev => (now_ms - prev) as f32,
            _ => return,
        };
        let (max_x, max_y, _, _) = self.scroll_bounds(sv);
        let (ox, oy) = self.tree.element_get_scroll_offset(sv);
        let (nx, vx2) = scroll_drag::scroll_motion_step(ox, vx, max_x, dt, &self.scroll_tuning);
        let (ny, vy2) = scroll_drag::scroll_motion_step(oy, vy, max_y, dt, &self.scroll_tuning);
        self.commit_scroll_offset(sv, nx, ny);
        // 軸は速度を持つかオーバースクロール中である限りアニメーション継続中
        // （バウンスの途中では一瞬速度ゼロを読むことがある）。
        let x_active = vx2 != 0.0 || nx < 0.0 || nx > max_x;
        let y_active = vy2 != 0.0 || ny < 0.0 || ny > max_y;
        self.scroll_motion = (x_active || y_active).then_some((sv, (vx2, vy2)));
    }

    pub fn on_resize(&mut self, width: f32, height: f32, scale: f32) {
        let metrics = resize_observer::canvas_resize_metrics(width, height, scale as f64);
        self.canvas.set_width(metrics.buffer_width);
        self.canvas.set_height(metrics.buffer_height);
        self.apply_resize(metrics);
    }

    fn apply_resize(&mut self, metrics: resize_observer::CanvasResizeMetrics) {
        // 退化した 0×0 の報告は無視する。デタッチ済み・`display:none`・未レイアウトの
        // canvas（ResizeObserver の最初の tick や、`getBoundingClientRect()` が 0 になる
        // ヘッドレステスト DOM）は一時的にボックスなしを報告する。ビューポートを 0 に潰すと
        // すべての `%` サイズボックスがゼロになり、ルートが何も覆わなくなって hit-test/focus/IME
        // が静かに止まる。しかも次の実 tick でレイアウトを作り直すだけ。直前のビューポートを維持する。
        if metrics.viewport_width <= 0.0 || metrics.viewport_height <= 0.0 {
            return;
        }
        self.tree
            .set_viewport(metrics.viewport_width, metrics.viewport_height);
        self.backend.resize(
            metrics.buffer_width,
            metrics.buffer_height,
            metrics.content_scale,
        );
        self.tree
            .on_resize(metrics.viewport_width, metrics.viewport_height);
        *self.last_viewport.borrow_mut() = (metrics.viewport_width, metrics.viewport_height);
    }

    pub fn register_listener(&mut self, element_id: f64, event_kind: u32) -> Result<f64, JsValue> {
        let kind = DocumentEventKind::from_u32(event_kind)
            .ok_or_else(|| JsValue::from_str(&format!("unknown event kind {event_kind}")))?;
        let id = self
            .tree
            .register_listener(element_id_from_f64(element_id), kind);
        Ok(id.to_u64() as f64)
    }

    /// `id` とその子孫の要素 id を返す。remove の前に Hayate へ問い合わせるために使う。
    pub fn element_subtree_ids(&self, id: f64) -> Vec<f64> {
        self.tree
            .subtree_element_ids(element_id_from_f64(id))
            .into_iter()
            .map(element_id_to_f64)
            .collect()
    }

    pub fn element_set_scroll_offset(&mut self, id: f64, x: f32, y: f32) {
        self.tree
            .element_set_scroll_offset(element_id_from_f64(id), x, y);
    }

    /// 要素の現在のスクロールオフセット `[x, y]`（未知のときは 0,0）。
    /// `element_set_scroll_offset` と対称で、ホストがタッチ駆動のスクロール位置を読み戻せる
    /// （ADR-0082）。
    pub fn element_get_scroll_offset(&self, id: f64) -> Box<[f32]> {
        let (x, y) = self.tree.element_get_scroll_offset(element_id_from_f64(id));
        vec![x, y].into_boxed_slice()
    }

    pub fn element_set_font_family(&mut self, id: f64, family: &str) {
        self.tree
            .element_set_font_family(element_id_from_f64(id), family);
    }

    /// `id` の継承可能なテキストスタイルプロパティを 1 つ以上 unset し、親からの継承に戻す
    /// （ADR-0047）。`kinds` はパックされた u32 配列: 0 = Color, 1 = FontSize, 2 = FontFamily。
    pub fn element_unset_style(&mut self, id: f64, kinds: &[u32]) -> Result<(), JsValue> {
        let parsed: Result<Vec<StylePropKind>, JsValue> = kinds
            .iter()
            .map(|&kind| unset_kind_from_u32(kind).map_err(|e| JsValue::from_str(&e)))
            .collect();
        self.tree
            .element_unset_style(element_id_from_f64(id), &parsed?);
        Ok(())
    }

    pub fn element_set_aria_label(&mut self, id: f64, label: &str) {
        self.tree
            .element_set_aria_label(element_id_from_f64(id), label);
    }

    pub fn element_set_role(&mut self, id: f64, role: &str) {
        self.tree.element_set_role(element_id_from_f64(id), role);
    }

    /// 生バイトからカスタムフォントを登録する。これ以降 family_name を
    /// `element_set_font_family` で使える。
    pub fn register_font_bytes(&mut self, family_name: &str, data: &[u8]) {
        self.tree.register_font(family_name, data.to_vec());
    }

    /// URL からフォントファイルを取得し `family_name` で登録する。
    pub async fn load_font_from_url(
        &mut self,
        family_name: String,
        url: String,
    ) -> Result<(), JsValue> {
        let bytes = fetch_bytes(&url).await?;
        self.tree.register_font(&family_name, bytes);
        Ok(())
    }

    /// アプリの `hayate.config.json` で宣言されたフォントをプリロードする。
    ///
    /// `{ family: string, url: string }` オブジェクトの JS 配列を受け取る。各フォントを
    /// 順次取得し、すべて登録されるまでブロックするので、最初の `render()` フレームが正しい
    /// フォントを使う（FOUT なし）。
    ///
    /// # Example (JS)
    /// ```js
    /// const cfg = await fetch('./hayate.config.json').then(r => r.json());
    /// await renderer.configure_fonts(cfg.fonts);
    /// ```
    pub async fn configure_fonts(&mut self, fonts: JsValue) -> Result<(), JsValue> {
        use js_sys::{Array, Reflect};
        let arr = Array::from(&fonts);
        for i in 0..arr.length() {
            let item = arr.get(i);
            let family = Reflect::get(&item, &JsValue::from_str("family"))?
                .as_string()
                .ok_or_else(|| JsValue::from_str("configure_fonts: missing 'family'"))?;
            let url = Reflect::get(&item, &JsValue::from_str("url"))?
                .as_string()
                .ok_or_else(|| JsValue::from_str("configure_fonts: missing 'url'"))?;
            let bytes = fetch_bytes(&url).await?;
            self.tree.register_font(&family, bytes);
        }
        Ok(())
    }

    /// フォントファイルに埋め込まれたファミリ名でフォントを読み込む。WIT の
    /// `element-load-font` エクスポートを支える。
    pub fn element_load_font(&mut self, data: &[u8]) {
        self.tree.register_font_bytes(data.to_vec());
    }

    /// 貼り付けテキストを特定の TextInput 要素に届ける。WIT 準拠（`element-paste`）。
    /// 暗黙フォーカスの `on_clipboard_paste` を置き換える。
    pub fn element_paste(&mut self, id: f64, text: &str) {
        self.tree.element_paste(element_id_from_f64(id), text);
    }

    /// フォーカス中の要素 id（f64）を返す。何もフォーカスされていなければ 0.0。
    /// JS は `element_get_text_content` と組み合わせてコピー/カットを実装できる。
    pub fn focused_element_id(&self) -> f64 {
        self.tree
            .focused_element()
            .map(element_id_to_f64)
            .unwrap_or(0.0)
    }

    /// 文書全体のテキスト選択がアクティブかどうか（ADR-0097）。これが true なら、要素が
    /// フォーカスされていなくてもホストはキーボードの選択操作（Ctrl/Cmd+A, Shift+矢印）を
    /// ディスパッチする（読み取り専用の Selection Region）。
    pub fn has_selection(&self) -> bool {
        self.tree.selection().is_some()
    }

    /// 直近のポインタ操作の物理デバイス。`PointerKind` のワイヤ値（`mouse=0`, `touch=1`,
    /// `pen=2`）。ホストが分岐できるようインタラクション単位で保持する。
    pub fn last_pointer_kind(&self) -> u32 {
        self.tree.last_pointer_kind().to_u32()
    }

    /// フォーカス中の要素へのキー押下を処理する。編集キーはアダプタの OS キーマップで
    /// [`EditIntent`] にマップされ、core の編集シーム（ADR-0103）経由で適用される。それ以外は
    /// 生の `on_key_down` 経路（非編集キーとアプリ向け `KeyDown` 通知）へ落ちる。
    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        if let Some(intent) = crate::edit_keymap::key_to_edit_intent(key, modifiers) {
            if let Some(focused) = self.tree.focused_element() {
                // Ctrl/Cmd+V: ブラウザのクリップボード読み取りは非同期で、core の同期
                // `Clipboard::read_text` シームでは扱えない。ここで
                // `navigator.clipboard.readText()` を開始し、解決したテキストを次フレームの
                // `element_paste` へ戻す（ADR-0097）。
                if intent == EditIntent::Paste {
                    self.spawn_clipboard_paste(focused);
                    return;
                }
                if self.tree.apply_edit_intent(focused, intent) {
                    return;
                }
            }
        }
        self.tree.on_key_down(key, modifiers);
    }

    /// `target` への Ctrl/Cmd+V のため非同期クリップボード読み取りを開始し、解決したテキストを
    /// 次の `render()` の `element_paste` 用にキューする。読み取りは、ブラウザがクリップボード
    /// アクセス許可に要求するユーザージェスチャの keydown 内で開始する（ADR-0097）。
    fn spawn_clipboard_paste(&mut self, target: ElementId) {
        let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard()) else {
            return;
        };
        let promise = clipboard.read_text();
        let queue = self.pending_paste.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(value) = JsFuture::from(promise).await {
                if let Some(text) = value.as_string() {
                    if !text.is_empty() {
                        queue.borrow_mut().push((target, text));
                    }
                }
            }
        });
    }

    /// フォーカス中の TextInput に印字可能なテキストが入力されたとき JS から呼ばれる。
    pub fn on_text_input(&mut self, id: f64, text: &str) {
        self.tree.on_text_input(element_id_from_f64(id), text);
    }

    /// IME 変換が開始したとき JS から呼ばれる。
    pub fn on_composition_start(&mut self, id: f64, text: &str) {
        self.tree
            .on_composition_start(element_id_from_f64(id), text);
    }

    /// IME のプリエディットが更新されたとき JS から呼ばれる。
    pub fn on_composition_update(&mut self, id: f64, text: &str) {
        self.tree
            .on_composition_update(element_id_from_f64(id), text);
    }

    /// IME のプリエディット更新時、EditContext `textformatupdate` の文節フォーマット範囲
    /// （ADR-0102）を伴って JS から呼ばれ、Canvas Mode が文節ごとの変換下線を描く。`formats` は
    /// フラットな `[start, end, weight, …]` の三つ組ストリーム（`text` へのバイトオフセット、
    /// `weight == 0` は細線、非ゼロは太線）。
    pub fn on_composition_update_formatted(&mut self, id: f64, text: &str, formats: &[u32]) {
        let clauses = hayate_core::CompositionClause::from_wire(formats);
        self.tree
            .on_composition_update_formatted(element_id_from_f64(id), text, clauses);
    }

    /// IME 変換が確定したとき JS から呼ばれる。
    pub fn on_composition_end(&mut self, id: f64, text: &str) {
        self.tree.on_composition_end(element_id_from_f64(id), text);
    }

    /// IME 用のカーソル文字境界（ADR-0069）。レイアウト空間の `[x, y, width, height]`。
    pub fn element_character_bounds(&self, id: f64) -> Box<[f32]> {
        let eid = element_id_from_f64(id);
        match self.tree.element_character_bounds(eid) {
            Some(b) => vec![b.x, b.y, b.width, b.height].into_boxed_slice(),
            None => vec![0.0, 0.0, 0.0, 0.0].into_boxed_slice(),
        }
    }

    /// 直近の `render()` で同期した最後の IME 文字境界。
    pub fn ime_character_bounds(&self) -> Box<[f32]> {
        let b = self.ime.last_bounds();
        vec![b.x, b.y, b.width, b.height].into_boxed_slice()
    }

    /// このフレームでソフトキーボードを上げるべきか。`text-input` がフォーカス中のときだけ
    /// true（ADR-0069）。JS ホストは true のときだけ `EditContext`（キーボードを上げる）を
    /// アタッチするので、非編集コンテンツへの普通のタップでは呼び出されない。
    pub fn ime_wants_keyboard(&self) -> bool {
        self.ime.visible()
    }

    pub fn element_set_text_content(&mut self, id: f64, text: &str) {
        self.tree
            .element_set_text_content(element_id_from_f64(id), text);
    }

    /// バッチ適用。Tsubame Canvas Mode がフレームごとに一度呼ぶ（ADR-0052）。`ops` は固定長
    /// レコードのフラットな Float64Array、`styles` は OP_SET_STYLE レコードが参照する
    /// style_packet の Float32Array、`texts` は OP_SET_TEXT レコードが参照する文字列テーブル。
    pub fn apply_mutations(
        &mut self,
        ops: &[f64],
        styles: &[f32],
        texts: js_sys::Array,
    ) -> Result<(), JsValue> {
        // 中立化した apply_mutations_batch（ADR-0112）は文字列テーブルを `&[String]` で
        // 受け取り、エラーを `String` で返す。Web 境界で js_sys::Array を変換し、
        // `String` エラーを `JsValue` へ写す。
        let texts: Vec<String> = texts
            .iter()
            .map(|v| v.as_string().unwrap_or_default())
            .collect();
        apply_mutations_batch(self, ops, styles, &texts).map_err(|e| JsValue::from_str(&e))
    }

    /// ライブツリーから編集可能なテキスト内容を返す。
    pub fn element_get_text_content(&self, id: f64) -> String {
        self.tree.element_get_text_content(element_id_from_f64(id))
    }

    /// ADR-0053: 配信行 `[listener_id, kind, ...fields]`。`fetch_font` はここで消費され、
    /// ホストへは配信されない。
    pub fn poll_events(&mut self) -> js_sys::Array {
        for event in self.tree.poll_events() {
            if let Event::FetchFont { family } = event {
                // レンダラを意識した調達（ADR-0043）。GPU 経路ではモノクロ絵文字ファミリを
                // COLR ビルドに格上げする。バイトは `family` 名で登録するので core のルーティングは
                // そのまま。
                if let Some(url) = font_url_for_renderer(&family, self.backend.kind()) {
                    let queue = self.font_queue.clone();
                    let failures = self.font_failure_queue.clone();
                    let attempts = self.font_fetch_attempts.clone();
                    let url = url.to_string();
                    wasm_bindgen_futures::spawn_local(async move {
                        match fetch_bytes(&url).await {
                            Ok(bytes) => queue.borrow_mut().push((family, bytes)),
                            Err(e) => {
                                web_sys::console::warn_1(&e);
                                // バックオフした後で失敗を報告し、core が再要求できるように
                                // する（リトライ予算を使い切るまで）。
                                let n = {
                                    let mut a = attempts.borrow_mut();
                                    let c = a.entry(family.clone()).or_insert(0);
                                    *c += 1;
                                    *c
                                };
                                let delay = FETCH_BACKOFF_BASE_MS
                                    .saturating_mul(1 << (n - 1).min(8))
                                    .min(FETCH_BACKOFF_MAX_MS);
                                backoff_sleep(delay).await;
                                failures.borrow_mut().push(family);
                            }
                        }
                    });
                } else {
                    web_sys::console::warn_1(&JsValue::from_str(&format!(
                        "FetchFont: no URL for \"{family}\""
                    )));
                }
            }
        }
        encode_deliveries(&self.tree.poll_deliveries())
    }

    /// JSON エンコードした AccessKit `TreeUpdate`（ADR-0041）。レイアウト前は null を返す。
    pub fn poll_accessibility(&self) -> JsValue {
        match self.tree.accessibility_update() {
            Some(update) => match serde_json::to_string(&update) {
                Ok(json) => JsValue::from_str(&json),
                Err(_) => JsValue::NULL,
            },
            None => JsValue::NULL,
        }
    }
}

impl ApplyMutationsHost for HayateElementRenderer {
    fn tree_mut(&mut self) -> &mut ElementTree {
        &mut self.tree
    }

    fn remove_subtree(&mut self, id: ElementId) {
        self.tree.element_remove(id);
    }

    fn apply_focus(&mut self, id: ElementId) {
        self.tree.on_focus(id);
    }

    fn apply_blur(&mut self, id: ElementId) {
        self.tree.on_blur(id);
    }
}

/// `Some(Color)` を `{r,g,b,a}` に、`None` を `null` に変換する。
fn color_to_js(color: Option<Color>) -> JsValue {
    let Some(c) = color else {
        return JsValue::NULL;
    };
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("r"), &JsValue::from_f64(c.r)).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("g"), &JsValue::from_f64(c.g)).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("b"), &JsValue::from_f64(c.b)).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("a"), &JsValue::from_f64(c.a)).unwrap();
    obj.into()
}

fn font_style_to_js(value: Option<FontStyleValue>) -> JsValue {
    match value {
        Some(FontStyleValue::Normal) => JsValue::from_str("normal"),
        Some(FontStyleValue::Italic) => JsValue::from_str("italic"),
        Some(FontStyleValue::Oblique) => JsValue::from_str("oblique"),
        None => JsValue::NULL,
    }
}

fn text_decoration_to_js(value: Option<TextDecorationValue>) -> JsValue {
    match value {
        Some(TextDecorationValue::None) => JsValue::from_str("none"),
        Some(TextDecorationValue::Underline) => JsValue::from_str("underline"),
        Some(TextDecorationValue::LineThrough) => JsValue::from_str("line-through"),
        None => JsValue::NULL,
    }
}

fn border_style_to_js(value: BorderStyleValue) -> JsValue {
    match value {
        BorderStyleValue::None => JsValue::from_str("none"),
        BorderStyleValue::Solid => JsValue::from_str("solid"),
        BorderStyleValue::Dashed => JsValue::from_str("dashed"),
    }
}

/// URL を取得し RGBA8 としてデコードする（PNG / JPEG / WebP 対応）。
async fn fetch_image(url: &str) -> Result<RenderImage, JsValue> {
    use js_sys::{ArrayBuffer, Uint8Array};

    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await?
        .dyn_into()?;
    let buf: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
    let bytes = Uint8Array::new(&buf).to_vec();

    let img = image::load_from_memory(&bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let raw = rgba.into_raw();

    Ok(RenderImage {
        data: Arc::from(raw.into_boxed_slice()),
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        width,
        height,
    })
}

/// ポインタ下で解決したカーソルからブラウザのカーソルを駆動する（ADR-0088 / ADR-0105）。
/// 生成済みの Hayate-CSS → browser-CSS マッパを再利用して `cursor` 値リストを単一ソースに
/// 保ち、body 全体ではなくポインタが乗っている canvas 要素自体に適用する。
fn apply_resolved_cursor(canvas: &HtmlCanvasElement, cursor: CursorValue) {
    let mut entries: Vec<(String, String)> = Vec::new();
    crate::generated::style_prop_css_entries(&StyleProp::Cursor(cursor), &mut entries);
    let Some((_, value)) = entries.into_iter().next() else {
        return;
    };
    let _ = canvas.style().set_property("cursor", &value);
}
