//! HTML Mode レンダラ（`HayateElementHtmlRenderer`）と DOM 実体化用の
//! サイドテーブル `HtmlNode`。レイアウトはブラウザ CSS が担う（ADR-0029）。
//!
//! 要素構造は `ElementTree` が保持し（ADR-0057）、描画上の正本は DOM。
//! `HtmlNode` は DOM 実体化に必要なもの（DOM ハンドルと保留中の text/src）
//! だけを持ち、再親付け／削除は第2のツリーではなく DOM から構造を読む。

use std::collections::HashMap;

use hayate_core::wire::MutationSink;
use hayate_core::{
    DocumentEventKind, ElementId, ElementKind, ElementTree, PseudoState, StyleProp, StylePropKind,
    UserSelectValue, ViewportCondition,
};
use wasm_bindgen::prelude::*;
use web_sys::{
    CssStyleRule, CssStyleSheet, Document, Element, HtmlElement, HtmlInputElement,
    HtmlStyleElement, HtmlTextAreaElement, Node, NodeList,
};

use crate::generated::encode_deliveries;
use crate::pseudo_style_dom::{
    pseudo_patch_rule_body, pseudo_state_css_priority, pseudo_state_css_suffix,
};
use crate::style_packet;
use crate::user_select::resolve_user_select;

use crate::shared::{document, element_id_from_f64, element_id_to_f64, fetch_bytes, kind_from_u32};

// ── 遅延コマンドキュー（ADR-0030、HTML Mode 専用 ADR-0037）────────
//
// HTML Mode では JS 向けの各 `element_*` ミューテータが `Command` をキューへ
// 積んで即座に戻る。キューを drain して適用する唯一のフラッシュ境界は
// `render()` で、DOM 変更をまとめてフレームあたり1回の reflow に抑える。
//
// Canvas Mode はキューを使わない（ADR-0037）。Tsubame が JS 側でフレームの
// 変更をまとめ `apply_mutations` 1回で渡すため、`HayateElementRenderer` の
// セッターは `ElementTree` へ即時適用する。

enum Command {
    SetText {
        id: ElementId,
        text: String,
    },
    SetSrc {
        id: ElementId,
        url: String,
    },
    SetDisabled {
        id: ElementId,
        disabled: bool,
    },
    /// ADR-0108 `user-select` 語彙（旧 `SetSelectable` boolean を置換）。
    SetUserSelect {
        id: ElementId,
        value: UserSelectValue,
    },
    Focus {
        id: ElementId,
    },
    Blur {
        id: ElementId,
    },
    SetMultiline {
        id: ElementId,
        multiline: bool,
    },
    SetStyle {
        id: ElementId,
        props: Vec<StyleProp>,
    },
    SetPseudoStyle {
        id: ElementId,
        state: hayate_core::PseudoState,
        props: Vec<StyleProp>,
    },
    UnsetStyle {
        id: ElementId,
        kind: StylePropKind,
    },
    SetTransform {
        id: ElementId,
        matrix: Option<[f64; 6]>,
    },
    SetScrollOffset {
        id: ElementId,
        x: f32,
        y: f32,
    },
    SetFontFamily {
        id: ElementId,
        family: String,
    },
    SetAriaLabel {
        id: ElementId,
        label: String,
    },
    SetRole {
        id: ElementId,
        role: String,
    },
    SetTextContent {
        id: ElementId,
        text: String,
    },
    AppendChild {
        parent: ElementId,
        child: ElementId,
    },
    InsertBefore {
        parent: ElementId,
        child: ElementId,
        before: ElementId,
    },
    Remove {
        id: ElementId,
    },
    SetRoot {
        id: ElementId,
    },
    /// HTML Mode 専用。確保済みスロットの DOM 要素を実体化する。Canvas Mode は
    /// `element_create` 内でツリーエントリを即時確保するためこのコマンドを出さない。
    HtmlCreate {
        id: ElementId,
        kind: ElementKind,
    },
}

/// 要素ごとの DOM 実体化レコード。構造は持たない。親子エッジは
/// `ElementTree`（イベント/スクロール）と DOM（描画）に存在する。
struct HtmlNode {
    kind: ElementKind,
    /// 遅延 `HtmlCreate` が `render()` でフラッシュされると `Some` になる。
    /// 初回フラッシュ前にキューされた操作は slotmap エントリは見えるが
    /// DOM 要素はまだ無い（ADR-0030）。
    dom: Option<Element>,
    text: Option<String>,
    src: Option<String>,
}

#[wasm_bindgen]
pub struct HayateElementHtmlRenderer {
    container: HtmlElement,
    /// 要素構造（親子・リスナ・バブル・スクロールオフセット）の唯一の所有者。
    /// HTML Mode では Taffy レイアウトを走らせない。
    tree: ElementTree,
    /// 要素 id をキーとする DOM 実体化サイドテーブル（構造は持たない）。
    nodes: HashMap<ElementId, HtmlNode>,
    root: Option<ElementId>,
    /// コンテナの CSS 背景色。HTML Mode は描画をブラウザへ委譲し、
    /// `set_background_color` で保存して `render(timestamp_ms)` のフラッシュ時に
    /// 1回だけ適用する。
    background_css: String,
    /// 各 `render()` の冒頭で適用される遅延変更（ADR-0030）。
    pending: Vec<Command>,
    /// 仕様順に並んだ擬似状態スタイルシート（`<style data-hayate-pseudo>`）。
    pseudo_style_el: HtmlStyleElement,
    /// `(element_id, pseudo_state)` ごとの `pseudo_style_el` 内ルールインデックス。
    pseudo_rule_keys: HashMap<(ElementId, PseudoState), u32>,
}

#[wasm_bindgen]
impl HayateElementHtmlRenderer {
    pub fn new(container: HtmlElement) -> Result<HayateElementHtmlRenderer, JsValue> {
        inject_baseline_stylesheet()?;
        let pseudo_style_el = ensure_pseudo_stylesheet()?;
        let style = container.style();
        style.set_property("position", "relative")?;
        style.set_property("overflow", "hidden")?;
        Ok(Self {
            container,
            tree: ElementTree::new(),
            nodes: HashMap::new(),
            root: None,
            background_css: "rgb(0,0,0)".to_string(),
            pending: Vec::new(),
            pseudo_style_el,
            pseudo_rule_keys: HashMap::new(),
        })
    }

    /// コンテナの CSS 背景色を次の `render()` のために保存する。
    /// `HayateElementRenderer::set_background_color` と対になり、同じセッターで
    /// どちらのモードも駆動できる。
    pub fn set_background_color(&mut self, r: f64, g: f64, b: f64) {
        self.background_css = format!(
            "rgb({},{},{})",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        );
    }

    /// HTML Mode のビューポートはブラウザ管理。Canvas レンダラとの API 互換の
    /// ために残してあり、Resize イベントを出すだけ。
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
        self.tree.on_resize(width, height);
    }

    /// 呼び出し側指定の ID で要素を登録し、DOM 生成をキューする。
    /// 実際の DOM 要素は次の `render()` で実体化される（ADR-0030）。
    pub fn element_create(&mut self, id: f64, kind: u32) -> Result<(), JsValue> {
        let k = kind_from_u32(kind)?;
        MutationSink::create(self, element_id_from_f64(id), k);
        Ok(())
    }

    // 命令的ミューテータ（`element_set_*` / `element_unset_style`）は撤去した（#439）。
    // HTML Mode の変更はすべて `apply_mutations`（中立 decode → 自身の `MutationSink`
    // 実装＝HtmlDeferred）を1本通る。構造系（create/append_child/insert_before/remove/
    // set_root）は API 互換のため命令的エントリを残すが、本体は `MutationSink` へ委譲し
    // 二重管理を避ける。

    pub fn element_append_child(&mut self, parent: f64, child: f64) {
        MutationSink::append_child(
            self,
            element_id_from_f64(parent),
            element_id_from_f64(child),
        );
    }

    pub fn element_insert_before(&mut self, parent: f64, child: f64, before: f64) {
        MutationSink::insert_before(
            self,
            element_id_from_f64(parent),
            element_id_from_f64(child),
            element_id_from_f64(before),
        );
    }

    pub fn element_remove(&mut self, id: f64) {
        MutationSink::remove(self, element_id_from_f64(id));
    }

    /// 直近の `render()` で確定したテキストを返す。キュー済みの
    /// `element_set_text` は次のフラッシュまで見えない（ADR-0030）。
    pub fn element_get_text(&self, id: f64) -> String {
        self.nodes
            .get(&element_id_from_f64(id))
            .and_then(|n| n.text.clone())
            .unwrap_or_default()
    }

    pub fn set_root(&mut self, id: f64) {
        MutationSink::set_root(self, element_id_from_f64(id));
    }

    /// キュー済みの要素変更を drain し、コンテナ背景色を更新する。新たに適用された
    /// スタイルの reflow はブラウザが1バッチで処理する。`timestamp_ms` は Canvas
    /// レンダラとの API 互換のために受け取るだけ（HTML Mode のカーソル点滅は
    /// ネイティブ `<input>` が担うので進めるものは無い）。
    pub fn render(&mut self, _timestamp_ms: f64) -> Result<(), JsValue> {
        self.flush_pending()?;
        self.container
            .style()
            .set_property("background-color", &self.background_css)?;
        Ok(())
    }

    // ── 入力配線 ─────────────────────────────────────────────────────
    // HTML Mode は Taffy を走らせないため、ヒットテストにレイアウトキャッシュを
    // 使えない。JS が `event.target` から `data-element-id` を読み、以下の
    // 明示ターゲット方式でディスパッチする。座標ベースの旧メソッドは Canvas Mode と
    // 共有する呼び出し側がコンパイルし続けられるよう no-op として残す。

    pub fn on_pointer_down(&mut self, target_id: f64, x: f32, y: f32) {
        let target = element_id_from_f64(target_id);
        if !self.nodes.contains_key(&target) {
            return;
        }
        self.tree.on_pointer_down_on(target, x, y);
    }

    pub fn on_pointer_up(&mut self, target_id: f64, _x: f32, _y: f32) {
        let explicit = element_id_from_f64(target_id);
        let fallback = self.nodes.contains_key(&explicit).then_some(explicit);
        self.tree.on_pointer_up_on(fallback);
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        let _ = self.tree.on_pointer_move_coords(x, y);
    }

    pub fn on_pointer_enter(&mut self, target_id: f64) {
        let target = element_id_from_f64(target_id);
        if !self.nodes.contains_key(&target) {
            return;
        }
        self.tree.on_hover_enter(target);
    }

    pub fn on_pointer_leave(&mut self, target_id: f64) {
        let target = element_id_from_f64(target_id);
        self.tree.on_hover_leave(target);
    }

    pub fn on_wheel(&mut self, target_id: f64, delta_x: f32, delta_y: f32) {
        let target = element_id_from_f64(target_id);
        if !self.nodes.contains_key(&target) {
            return;
        }
        if let Some(sv) = self.tree.apply_wheel_delta(target, delta_x, delta_y) {
            let (x, y) = self.tree.element_get_scroll_offset(sv);
            self.flush_set_scroll_offset(sv, x, y);
        }
        self.tree.on_wheel(target, delta_x, delta_y);
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
        self.tree.on_resize(width, height);
    }

    pub fn register_listener(&mut self, element_id: f64, event_kind: u32) -> Result<f64, JsValue> {
        let kind = DocumentEventKind::from_u32(event_kind)
            .ok_or_else(|| JsValue::from_str(&format!("unknown event kind {event_kind}")))?;
        let id = self
            .tree
            .register_listener(element_id_from_f64(element_id), kind);
        Ok(id.to_u64() as f64)
    }

    /// バッチ適用。Tsubame と Canvas Mode を揃える唯一のミューテーション入口（ADR-0052）。
    /// 中立 decode（`hayate_core::wire`）を自身の `MutationSink` 実装（HtmlDeferred）で
    /// 駆動し、各 op を遅延コマンドへ積む。実 DOM への反映は `render()` の1バッチ
    /// flush（ADR-0030）。`texts` は `OP_SET_TEXT` 等が参照する文字列テーブル。
    pub fn apply_mutations(
        &mut self,
        ops: &[f64],
        styles: &[f32],
        texts: js_sys::Array,
        draws: &[f32],
    ) -> Result<(), JsValue> {
        let texts: Vec<String> = texts
            .iter()
            .map(|v| v.as_string().unwrap_or_default())
            .collect();
        hayate_core::wire::apply_mutations_to_sink(self, ops, styles, &texts, draws)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Web フォントを CSS `@font-face` で登録する。HTML Mode はブラウザがテキストを
    /// 描画するため、フォント登録はドキュメントの CSS エンジンへ委譲する。
    pub fn register_font_bytes(&mut self, family_name: &str, data: &[u8]) {
        let _ = inject_font_face(family_name, data);
    }

    pub async fn load_font_from_url(
        &mut self,
        family_name: String,
        url: String,
    ) -> Result<(), JsValue> {
        let bytes = fetch_bytes(&url).await?;
        let _ = inject_font_face(&family_name, &bytes);
        Ok(())
    }

    /// `hayate.config.json` で宣言されたフォントを初回描画前にプリロードする。
    /// HTML Mode は各フォントを CSS `@font-face` ルールとして注入する。
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
            let _ = inject_font_face(&family, &bytes);
        }
        Ok(())
    }

    /// WIT `element-load-font`: HTML Mode はフォントバイト列から family 名を
    /// 読めない（JS 側に Parley FontContext が無い）。合成 family 名で `@font-face`
    /// として登録し、少なくともデータ URL をドキュメントに常駐させる。特定の
    /// family 名が必要なら `register_font_bytes` を使い続けること。
    pub fn element_load_font(&mut self, data: &[u8]) {
        // 内容ハッシュから安定かつ一意な family 名を生成する。
        let mut h: u64 = 0xcbf29ce484222325;
        for b in data {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let family = format!("hayate-font-{h:016x}");
        let _ = inject_font_face(&family, data);
    }

    /// WIT `element-paste`: 貼り付けテキストを特定の TextInput へ届け、TextInput
    /// イベントを発火する。ネイティブ `<input>` の value への反映は DOM の `paste`
    /// イベントで別途行われる。
    pub fn element_paste(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.tree.element_paste(eid, text);
        }
    }

    /// WIT `element-get-bounds`: 要素の CSS バウンディングボックス
    /// [x, y, width, height] をコンテナ相対ピクセルで返す。未レイアウトの場合は
    /// すべて 0 を返す。
    pub fn element_get_bounds(&self, id: f64) -> Box<[f32]> {
        let eid = element_id_from_f64(id);
        let dom = match self.nodes.get(&eid).and_then(|n| n.dom.as_ref()) {
            Some(d) => d,
            None => return vec![0.0, 0.0, 0.0, 0.0].into_boxed_slice(),
        };
        let html_el = match dom.dyn_ref::<HtmlElement>() {
            Some(e) => e,
            None => return vec![0.0, 0.0, 0.0, 0.0].into_boxed_slice(),
        };
        // offsetLeft/Top は offsetParent 相対。コンテナを根とするツリーでは
        // これが WIT の「canvas 座標」の期待に一致する。
        vec![
            html_el.offset_left() as f32,
            html_el.offset_top() as f32,
            html_el.offset_width() as f32,
            html_el.offset_height() as f32,
        ]
        .into_boxed_slice()
    }

    pub fn focused_element_id(&self) -> f64 {
        self.tree
            .focused_element()
            .map(element_id_to_f64)
            .unwrap_or(0.0)
    }

    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        self.tree.on_key_down(key, modifiers);
    }

    pub fn on_text_input(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.tree.on_text_input(eid, text);
        }
    }

    pub fn on_composition_start(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.tree.on_composition_start(eid, text);
        }
    }

    pub fn on_composition_update(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.tree.on_composition_update(eid, text);
        }
    }

    pub fn on_composition_end(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.tree.on_composition_end(eid, text);
        }
    }

    /// 直近の `render()` で確定した編集可能テキスト内容を返す。TextInput では
    /// ユーザー入力を既に反映しているライブ DOM 値へフォールスルーする
    /// （キュー駆動ではなくブラウザ駆動）。キュー済みの
    /// `element_set_text_content` は次のフラッシュまで見えない（ADR-0030）。
    pub fn element_get_text_content(&self, id: f64) -> String {
        let eid = element_id_from_f64(id);
        let n = match self.nodes.get(&eid) {
            Some(n) => n,
            None => return String::new(),
        };
        if let Some(dom) = n.dom.as_ref() {
            if let Some(value) = text_field_value(dom) {
                return value;
            }
        }
        n.text.clone().unwrap_or_default()
    }

    /// 画像の `src` を URL に設定する。取得とデコードはブラウザが行う。
    /// 次の `render()` を待たずブラウザの fetch を始められるよう `src` は DOM へ
    /// 即時適用し、読み取りが新 URL を即座に観測できるよう slotmap ミラーも更新する。
    pub async fn load_image(&mut self, id: f64, url: String) -> Result<(), JsValue> {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get_mut(&eid) {
            if n.kind == ElementKind::Image {
                n.src = Some(url.clone());
                if let Some(dom) = n.dom.as_ref() {
                    let _ = dom.set_attribute("src", &url);
                }
            }
        }
        Ok(())
    }

    /// 配信行 `[listener_id, kind, ...fields]`（ADR-0053）。
    pub fn poll_events(&mut self) -> js_sys::Array {
        encode_deliveries(&self.tree.poll_deliveries())
    }
}

/// HtmlDeferred sink（ADR-0030 / #439）。中立 decode が呼ぶ意味的ミューテーションを
/// 遅延コマンドキューへ積む（構造・スクロール・フォーカス・disabled はコアツリーへ即時
/// 反映してイベント/読み戻しの正本を保つ）。実 DOM への反映は `render()` の1バッチ
/// flush で行う。Canvas の即時 `TreeSink` と同じ op ストリームを消費し、差分は
/// 「即時木適用 vs 遅延 DOM enqueue」だけ。
impl MutationSink for HayateElementHtmlRenderer {
    fn create(&mut self, id: ElementId, kind: ElementKind) {
        self.tree.element_create(id.to_u64(), kind);
        self.nodes.insert(
            id,
            HtmlNode {
                kind,
                dom: None,
                text: None,
                src: None,
            },
        );
        self.pending.push(Command::HtmlCreate { id, kind });
    }

    fn set_root(&mut self, id: ElementId) {
        self.tree.set_root(id);
        self.pending.push(Command::SetRoot { id });
    }

    fn append_child(&mut self, parent: ElementId, child: ElementId) {
        self.tree.element_append_child(parent, child);
        self.pending.push(Command::AppendChild { parent, child });
    }

    fn insert_before(&mut self, parent: ElementId, child: ElementId, before: ElementId) {
        self.tree.element_insert_before(parent, child, before);
        self.pending.push(Command::InsertBefore {
            parent,
            child,
            before,
        });
    }

    fn remove(&mut self, id: ElementId) {
        self.tree.element_remove(id);
        self.pending.push(Command::Remove { id });
    }

    fn set_text(&mut self, id: ElementId, text: &str) {
        self.pending.push(Command::SetText {
            id,
            text: text.to_string(),
        });
    }

    fn set_text_content(&mut self, id: ElementId, text: &str) {
        self.pending.push(Command::SetTextContent {
            id,
            text: text.to_string(),
        });
    }

    fn set_src(&mut self, id: ElementId, url: &str) {
        self.pending.push(Command::SetSrc {
            id,
            url: url.to_string(),
        });
    }

    fn set_disabled(&mut self, id: ElementId, disabled: bool) {
        self.tree.element_set_disabled(id, disabled);
        self.pending.push(Command::SetDisabled { id, disabled });
    }

    fn set_multiline(&mut self, id: ElementId, multiline: bool) {
        self.pending.push(Command::SetMultiline { id, multiline });
    }

    fn set_style(&mut self, id: ElementId, props: Vec<StyleProp>) {
        self.pending.push(Command::SetStyle { id, props });
    }

    fn set_pseudo_style(&mut self, id: ElementId, state: PseudoState, props: Vec<StyleProp>) {
        self.pending
            .push(Command::SetPseudoStyle { id, state, props });
    }

    fn unset_style(&mut self, id: ElementId, kind: StylePropKind) {
        self.pending.push(Command::UnsetStyle { id, kind });
    }

    fn set_transform(&mut self, id: ElementId, matrix: Option<[f64; 6]>) {
        self.pending.push(Command::SetTransform { id, matrix });
    }

    fn set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
        self.tree.element_set_scroll_offset(id, x, y);
        self.pending.push(Command::SetScrollOffset { id, x, y });
    }

    fn set_user_select(&mut self, id: ElementId, value: UserSelectValue) {
        self.pending.push(Command::SetUserSelect { id, value });
    }

    fn apply_focus(&mut self, id: ElementId) {
        self.tree.on_focus(id);
        self.pending.push(Command::Focus { id });
    }

    fn apply_blur(&mut self, id: ElementId) {
        self.tree.on_blur(id);
        self.pending.push(Command::Blur { id });
    }

    fn set_font_family(&mut self, id: ElementId, family: &str) {
        self.pending.push(Command::SetFontFamily {
            id,
            family: family.to_string(),
        });
    }

    fn set_draw(&mut self, _id: ElementId, _commands: Vec<hayate_core::DrawCommand>) {
        // HTML Mode は v1 で draw 非対応（PRD #723 / ADR-0141: Tsubame init から到達
        // 不能な dead path のため、生きた経路を先に揃える）。decode は受けて捨てる。
    }

    fn set_aria_label(&mut self, id: ElementId, label: &str) {
        self.pending.push(Command::SetAriaLabel {
            id,
            label: label.to_string(),
        });
    }

    fn set_role(&mut self, id: ElementId, role: &str) {
        self.pending.push(Command::SetRole {
            id,
            role: role.to_string(),
        });
    }

    fn set_style_variant(&mut self, id: ElementId, condition: ViewportCondition, prop: StyleProp) {
        // HTML Mode はレイアウトをブラウザへ委譲し viewport variant の DOM 反映を持たない。
        // 読み戻し/イベントの正本としてコアツリーにだけ適用する。
        self.tree.element_set_style_variant(id, condition, prop);
    }
}

impl HayateElementHtmlRenderer {
    /// 保留コマンドキューを drain し、各変更を DOM と slotmap に適用する。
    /// `render()`（唯一のフラッシュ境界、ADR-0030）から呼ばれる。
    fn flush_pending(&mut self) -> Result<(), JsValue> {
        let commands = std::mem::take(&mut self.pending);
        for cmd in commands {
            self.apply_command(cmd)?;
        }
        Ok(())
    }

    fn apply_command(&mut self, cmd: Command) -> Result<(), JsValue> {
        match cmd {
            Command::HtmlCreate { id, kind } => self.flush_create(id, kind)?,
            Command::SetText { id, text } => self.flush_set_text(id, &text),
            Command::SetSrc { id, url } => self.flush_set_src(id, &url),
            Command::SetDisabled { id, disabled } => self.flush_set_disabled(id, disabled),
            Command::SetUserSelect { id, value } => self.flush_set_user_select(id, value),
            Command::Focus { id } => self.flush_focus(id),
            Command::Blur { id } => self.flush_blur(id),
            Command::SetMultiline { id, multiline } => self.flush_set_multiline(id, multiline)?,
            Command::SetStyle { id, props } => self.flush_set_style(id, &props)?,
            Command::SetPseudoStyle { id, state, props } => {
                self.tree.element_set_pseudo_style(id, state, &props);
                self.flush_set_pseudo_style(id, state, &props)?;
            }
            Command::UnsetStyle { id, kind } => self.flush_unset_style(id, kind),
            Command::SetTransform { id, matrix } => self.flush_set_transform(id, matrix),
            Command::SetScrollOffset { id, x, y } => self.flush_set_scroll_offset(id, x, y),
            Command::SetFontFamily { id, family } => self.flush_set_font_family(id, &family),
            Command::SetAriaLabel { id, label } => self.flush_set_aria_label(id, &label),
            Command::SetRole { id, role } => self.flush_set_role(id, &role),
            Command::SetTextContent { id, text } => self.flush_set_text_content(id, &text),
            Command::AppendChild { parent, child } => self.flush_append_child(parent, child),
            Command::InsertBefore {
                parent,
                child,
                before,
            } => {
                self.flush_insert_before(parent, child, before);
            }
            Command::Remove { id } => self.flush_remove(id),
            Command::SetRoot { id } => self.flush_set_root(id),
        }
        Ok(())
    }

    fn flush_create(&mut self, id: ElementId, kind: ElementKind) -> Result<(), JsValue> {
        // スロットは `element_create` で即時挿入済み。無ければ後続のキュー済み
        // `Remove` で削除されたということなので静かにスキップする。
        if !self.nodes.contains_key(&id) {
            return Ok(());
        }
        let dom = create_dom_for_kind(&document(), kind)?;
        apply_kind_baseline(&dom, kind)?;
        dom.set_attribute("data-element-id", &format!("{}", id.to_u64()))?;
        if let Some(n) = self.nodes.get_mut(&id) {
            n.dom = Some(dom.clone());
        }
        // 自動ルートの挙動を維持する。ルートが無いときに最初に生成された要素が
        // ルートとなり、コンテナにマウントされる。
        if self.root.is_none() {
            self.root = Some(id);
            self.container.append_child(&dom)?;
        }
        Ok(())
    }

    fn flush_set_text(&mut self, id: ElementId, text: &str) {
        let n = match self.nodes.get_mut(&id) {
            Some(n) => n,
            None => return,
        };
        n.text = Some(text.to_string());
        let dom = match n.dom.as_ref() {
            Some(d) => d,
            None => return,
        };
        match n.kind {
            ElementKind::TextInput => {
                set_text_field_value(dom, text);
            }
            _ => {
                if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
                    html_el.set_inner_text(text);
                }
            }
        }
    }

    fn flush_set_src(&mut self, id: ElementId, url: &str) {
        let n = match self.nodes.get_mut(&id) {
            Some(n) => n,
            None => return,
        };
        n.src = Some(url.to_string());
        if n.kind == ElementKind::Image {
            if let Some(dom) = n.dom.as_ref() {
                let _ = dom.set_attribute("src", url);
            }
        }
    }

    /// ADR-0108 `user-select` を CSS へ反映する。明示値を要素種別の UA 既定で解決して
    /// 書き出す（旧 boolean `selectable` 橋渡しを撤去し、wire 語彙を直接消費する）。
    fn flush_set_user_select(&mut self, id: ElementId, value: UserSelectValue) {
        let (kind, dom) = match self.nodes.get(&id) {
            Some(n) => (n.kind, n.dom.clone()),
            None => return,
        };
        let dom = match dom {
            Some(d) => d,
            None => return,
        };
        if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            let css = resolve_user_select(kind, Some(value));
            let style = html_el.style();
            let _ = style.set_property("user-select", css);
            let _ = style.set_property("-webkit-user-select", css);
        }
    }

    /// `disabled` を DOM 属性へ反映する（input / button 等）。コアツリーは sink 側で
    /// 既に更新済み。
    fn flush_set_disabled(&mut self, id: ElementId, disabled: bool) {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return,
        };
        if disabled {
            let _ = dom.set_attribute("disabled", "");
        } else {
            let _ = dom.remove_attribute("disabled");
        }
    }

    /// フォーカス op をブラウザ DOM へ反映する。コアツリーのフォーカス状態は sink 側で
    /// 更新済み。
    fn flush_focus(&mut self, id: ElementId) {
        if let Some(html_el) = self
            .nodes
            .get(&id)
            .and_then(|n| n.dom.as_ref())
            .and_then(|d| d.dyn_ref::<HtmlElement>())
        {
            let _ = html_el.focus();
        }
    }

    fn flush_blur(&mut self, id: ElementId) {
        if let Some(html_el) = self
            .nodes
            .get(&id)
            .and_then(|n| n.dom.as_ref())
            .and_then(|d| d.dyn_ref::<HtmlElement>())
        {
            let _ = html_el.blur();
        }
    }

    /// TextInput の実体化要素を `<input>` と `<textarea>` で入れ替え、ブラウザ
    /// ネイティブの Enter 挙動を `multiline` プロパティに合わせる。textarea は
    /// キャレットに改行を入れ、input は送信する。入れ替えを跨いでライブ値と
    /// 解決済みインラインスタイルを引き継ぐ。
    fn flush_set_multiline(&mut self, id: ElementId, multiline: bool) -> Result<(), JsValue> {
        // 読み取りがレンダラ間で一致するようコアツリーを正本に保つ。
        self.tree.element_set_multiline(id, multiline);
        let (kind, dom) = match self.nodes.get(&id) {
            Some(n) => (n.kind, n.dom.clone()),
            None => return Ok(()),
        };
        if kind != ElementKind::TextInput {
            return Ok(());
        }
        let old = match dom {
            Some(d) => d,
            None => return Ok(()),
        };
        let is_textarea = old.dyn_ref::<HtmlTextAreaElement>().is_some();
        if is_textarea == multiline {
            return Ok(()); // 既に正しい要素
        }
        let doc = document();
        let new_el = doc.create_element(if multiline { "textarea" } else { "input" })?;
        apply_kind_baseline(&new_el, ElementKind::TextInput)?;
        if !multiline {
            new_el.set_attribute("type", "text")?;
        }
        new_el.set_attribute("data-element-id", &format!("{}", id.to_u64()))?;
        // 入れ替えを跨いでライブ値を保つ（まず DOM、次にミラー）。
        let value =
            text_field_value(&old).or_else(|| self.nodes.get(&id).and_then(|n| n.text.clone()));
        if let Some(v) = value.as_deref() {
            set_text_field_value(&new_el, v);
        }
        // 解決済みインラインスタイル（baseline + user + selection）を引き継ぐ。
        if let (Some(old_h), Some(new_h)) = (
            old.dyn_ref::<HtmlElement>(),
            new_el.dyn_ref::<HtmlElement>(),
        ) {
            let _ = new_h.style().set_css_text(&old_h.style().css_text());
        }
        if let Some(parent) = old.parent_node() {
            parent.replace_child(&new_el, &old)?;
        }
        if let Some(n) = self.nodes.get_mut(&id) {
            n.dom = Some(new_el);
        }
        Ok(())
    }

    fn flush_set_style(&mut self, id: ElementId, props: &[StyleProp]) -> Result<(), JsValue> {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return Ok(()),
        };
        if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            style_packet::apply_props_to_dom(&html_el.style(), props)?;
        }
        Ok(())
    }

    fn flush_set_pseudo_style(
        &mut self,
        id: ElementId,
        state: PseudoState,
        props: &[StyleProp],
    ) -> Result<(), JsValue> {
        let kind = match self.nodes.get(&id).map(|n| n.kind) {
            Some(k) => k,
            None => return Ok(()),
        };
        let body = pseudo_patch_rule_body(kind, props);
        if body.is_empty() {
            self.remove_pseudo_rule(id, state)?;
            return Ok(());
        }

        let sheet = match self.pseudo_style_el.sheet() {
            Some(s) => s.dyn_into::<CssStyleSheet>().ok(),
            None => None,
        };
        let sheet = match sheet {
            Some(s) => s,
            None => return Ok(()),
        };

        let selector = format!(
            "[data-element-id=\"{}\"]{}",
            id.to_u64(),
            pseudo_state_css_suffix(state)
        );
        let css_text = format!("{selector}{{{body}}}");
        let key = (id, state);
        let priority = pseudo_state_css_priority(state);

        if let Some(&index) = self.pseudo_rule_keys.get(&key) {
            if let Ok(rules) = sheet.css_rules() {
                if let Some(rule) = rules.item(index) {
                    if let Ok(style_rule) = rule.dyn_into::<CssStyleRule>() {
                        style_rule.style().set_css_text(&body);
                        return Ok(());
                    }
                }
            }
            sheet.delete_rule(index)?;
            self.bump_pseudo_rule_indices(index, -1);
            self.pseudo_rule_keys.remove(&key);
        }

        let index = insertion_index_for_pseudo_band(&sheet, priority)?;
        sheet.insert_rule_with_index(&css_text, index)?;
        self.bump_pseudo_rule_indices(index, 1);
        self.pseudo_rule_keys.insert(key, index);
        Ok(())
    }

    fn remove_pseudo_rule(&mut self, id: ElementId, state: PseudoState) -> Result<(), JsValue> {
        let key = (id, state);
        let index = match self.pseudo_rule_keys.remove(&key) {
            Some(i) => i,
            None => return Ok(()),
        };
        if let Some(sheet) = self
            .pseudo_style_el
            .sheet()
            .and_then(|s| s.dyn_into::<CssStyleSheet>().ok())
        {
            let _ = sheet.delete_rule(index);
            self.bump_pseudo_rule_indices(index, -1);
        }
        Ok(())
    }

    fn remove_all_pseudo_rules_for(&mut self, id: ElementId) -> Result<(), JsValue> {
        for state in [PseudoState::Focus, PseudoState::Hover, PseudoState::Active] {
            self.remove_pseudo_rule(id, state)?;
        }
        Ok(())
    }

    fn bump_pseudo_rule_indices(&mut self, from: u32, delta: i32) {
        for index in self.pseudo_rule_keys.values_mut() {
            if *index >= from {
                *index = (*index as i32 + delta) as u32;
            }
        }
    }

    fn flush_unset_style(&mut self, id: ElementId, kind: StylePropKind) {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return,
        };
        if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            let style = html_el.style();
            let property = match kind {
                StylePropKind::Color => "color",
                StylePropKind::FontSize => "font-size",
                StylePropKind::FontFamily => "font-family",
                StylePropKind::FontWeight => "font-weight",
                _ => return,
            };
            let _ = style.remove_property(property);
        }
    }

    fn flush_set_transform(&mut self, id: ElementId, matrix: Option<[f64; 6]>) {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return,
        };
        let html_el = match dom.dyn_ref::<HtmlElement>() {
            Some(e) => e,
            None => return,
        };
        let style = html_el.style();
        match matrix {
            Some(m) => {
                let css = format!(
                    "matrix({},{},{},{},{},{})",
                    m[0], m[1], m[2], m[3], m[4], m[5]
                );
                let _ = style.set_property("transform", &css);
            }
            None => {
                let _ = style.set_property("transform", "none");
            }
        }
    }

    fn flush_set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
        if let Some(dom) = self.nodes.get(&id).and_then(|n| n.dom.as_ref()) {
            dom.set_scroll_left(x as i32);
            dom.set_scroll_top(y as i32);
        }
    }

    fn flush_set_font_family(&mut self, id: ElementId, family: &str) {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return,
        };
        if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            let _ = html_el.style().set_property("font-family", family);
        }
    }

    fn flush_set_aria_label(&mut self, id: ElementId, label: &str) {
        if let Some(dom) = self.nodes.get(&id).and_then(|n| n.dom.as_ref()) {
            let _ = dom.set_attribute("aria-label", label);
        }
    }

    fn flush_set_role(&mut self, id: ElementId, role: &str) {
        if let Some(dom) = self.nodes.get(&id).and_then(|n| n.dom.as_ref()) {
            let _ = dom.set_attribute("role", role);
        }
    }

    fn flush_set_text_content(&mut self, id: ElementId, text: &str) {
        let n = match self.nodes.get_mut(&id) {
            Some(n) => n,
            None => return,
        };
        n.text = Some(text.to_string());
        let dom = match n.dom.as_ref() {
            Some(d) => d,
            None => return,
        };
        if !set_text_field_value(dom, text) {
            if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
                html_el.set_inner_text(text);
            }
        }
    }

    fn flush_append_child(&mut self, pid: ElementId, cid: ElementId) {
        if !self.nodes.contains_key(&pid) || !self.nodes.contains_key(&cid) {
            return;
        }
        // `append_child` はノードを移動し、既存の DOM 親から切り離す。
        let parent_dom = self.nodes[&pid].dom.clone();
        let child_dom = self.nodes[&cid].dom.clone();
        if let (Some(p), Some(c)) = (parent_dom, child_dom) {
            let _ = p.append_child(c.as_ref());
        }
    }

    fn flush_insert_before(&mut self, pid: ElementId, cid: ElementId, bid: ElementId) {
        if !self.nodes.contains_key(&pid)
            || !self.nodes.contains_key(&cid)
            || !self.nodes.contains_key(&bid)
        {
            return;
        }
        let parent_dom = self.nodes[&pid].dom.clone();
        let child_dom = self.nodes[&cid].dom.clone();
        let before_dom = self.nodes[&bid].dom.clone();
        let (Some(p), Some(c), Some(b)) = (parent_dom, child_dom, before_dom) else {
            return;
        };
        // `before` は `parent` の子でなければならない。そうでなければ append に
        // フォールバックする（従来の構造ミラーのガードと同じ）。
        let before_is_child = b
            .parent_node()
            .is_some_and(|pn| pn.is_same_node(Some(p.as_ref())));
        if before_is_child {
            let _ = p
                .unchecked_ref::<Node>()
                .insert_before(c.as_ref(), Some(b.as_ref()));
        } else {
            let _ = p.append_child(c.as_ref());
        }
    }

    fn flush_remove(&mut self, target: ElementId) {
        if !self.nodes.contains_key(&target) {
            return;
        }
        let _ = self.remove_all_pseudo_rules_for(target);
        // DOM サブツリーが構造の正本（ADR-0029）。切り離す前に破棄する要素 id を
        // 集め、`remove_child` でカスケード削除する。
        let mut subtree = vec![target];
        if let Some(top_dom) = self.nodes[&target].dom.clone() {
            subtree.extend(descendant_element_ids(&top_dom));
            if let Some(parent_dom) = top_dom.parent_node() {
                let _ = parent_dom.remove_child(top_dom.as_ref());
            }
        }
        for id in subtree {
            self.nodes.remove(&id);
        }
        if self.root == Some(target) {
            self.root = None;
        }
        // 削除ノードのポインタ状態は `element_remove` で即時クリア済み。
    }

    fn flush_set_root(&mut self, new_root: ElementId) {
        if !self.nodes.contains_key(&new_root) {
            return;
        }
        // 直前のルートがあればコンテナから切り離す。
        if let Some(prev) = self.root {
            if prev != new_root {
                if let Some(prev_dom) = self.nodes[&prev].dom.clone() {
                    let _ = self.container.remove_child(prev_dom.as_ref());
                }
            }
        }
        // `append_child` は新ルートを以前の親から外し、コンテナにマウントする。
        if let Some(dom) = self.nodes[&new_root].dom.clone() {
            let _ = self.container.append_child(dom.as_ref());
        }
        self.root = Some(new_root);
    }
}

/// `data-element-id` を持つ `top` の子孫の要素 id。HTML Mode では DOM サブツリーが
/// 構造の正本（ADR-0029）なので、削除は第2のツリーではなく DOM から読む。
fn descendant_element_ids(top: &Element) -> Vec<ElementId> {
    let mut ids = Vec::new();
    let list: NodeList = match top.query_selector_all("[data-element-id]") {
        Ok(list) => list,
        Err(_) => return ids,
    };
    for i in 0..list.length() {
        if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
            if let Some(raw) = el.get_attribute("data-element-id") {
                if let Ok(v) = raw.parse::<u64>() {
                    ids.push(ElementId::from_u64(v));
                }
            }
        }
    }
    ids
}

fn ensure_pseudo_stylesheet() -> Result<HtmlStyleElement, JsValue> {
    let doc = document();
    if let Some(existing) = doc.get_element_by_id("hayate-pseudo") {
        return existing
            .dyn_into::<HtmlStyleElement>()
            .map_err(|_| JsValue::from_str("hayate-pseudo is not a style element"));
    }
    let head = doc.head().ok_or("no head")?;
    let style_el = doc
        .create_element("style")?
        .dyn_into::<HtmlStyleElement>()?;
    style_el.set_id("hayate-pseudo");
    let _ = style_el.set_attribute("data-hayate-pseudo", "");
    head.append_child(&style_el)?;
    Ok(style_el)
}

fn pseudo_priority_from_selector(selector: &str) -> u32 {
    if selector.ends_with(":focus") {
        return pseudo_state_css_priority(PseudoState::Focus);
    }
    if selector.ends_with(":hover") {
        return pseudo_state_css_priority(PseudoState::Hover);
    }
    if selector.ends_with(":active") {
        return pseudo_state_css_priority(PseudoState::Active);
    }
    0
}

fn insertion_index_for_pseudo_band(sheet: &CssStyleSheet, priority: u32) -> Result<u32, JsValue> {
    let rules = sheet.css_rules()?;
    for i in 0..rules.length() {
        if let Some(rule) = rules.item(i) {
            if let Ok(style_rule) = rule.dyn_into::<CssStyleRule>() {
                if pseudo_priority_from_selector(&style_rule.selector_text()) > priority {
                    return Ok(i);
                }
            }
        }
    }
    Ok(rules.length())
}

fn create_dom_for_kind(doc: &Document, kind: ElementKind) -> Result<Element, JsValue> {
    let tag = match kind {
        ElementKind::Image => "img",
        ElementKind::TextInput => "input",
        ElementKind::Button => "button",
        _ => "div",
    };
    let el = doc.create_element(tag)?;
    if kind == ElementKind::TextInput {
        let _ = el.set_attribute("type", "text");
    }
    Ok(el)
}

/// テキスト入力 DOM ノードの編集可能な値を読む。単一行 `<input>` でも複数行
/// `<textarea>` でも対応する。どちらでもない場合は `None` を返す。
fn text_field_value(dom: &Element) -> Option<String> {
    if let Some(input) = dom.dyn_ref::<HtmlInputElement>() {
        Some(input.value())
    } else {
        dom.dyn_ref::<HtmlTextAreaElement>()
            .map(|area| area.value())
    }
}

/// テキスト入力 DOM ノード（`<input>` か `<textarea>`）の編集可能な値を書く。
/// テキストフィールドだったかを返す（呼び出し側がフォールバックできるよう）。
fn set_text_field_value(dom: &Element, text: &str) -> bool {
    if let Some(input) = dom.dyn_ref::<HtmlInputElement>() {
        input.set_value(text);
        true
    } else if let Some(area) = dom.dyn_ref::<HtmlTextAreaElement>() {
        area.set_value(text);
        true
    } else {
        false
    }
}

/// 要素種別ごとのベースライン CSS。`element_set_style` 経由のユーザースタイルが
/// きれいに上書きできるよう最小限に保つ。React Native Web の resetStyle に倣い、
/// 予測可能なボックスモデルで継承による意外を避ける。
fn apply_kind_baseline(el: &Element, kind: ElementKind) -> Result<(), JsValue> {
    let html_el = match el.dyn_ref::<HtmlElement>() {
        Some(e) => e,
        None => return Ok(()),
    };
    let style = html_el.style();
    style.set_property("box-sizing", "border-box")?;
    style.set_property("position", "relative")?;
    style.set_property("margin", "0")?;
    style.set_property("padding", "0")?;
    style.set_property("border", "0 solid black")?;
    style.set_property("min-width", "0")?;
    style.set_property("min-height", "0")?;
    // 選択領域の既定: `selectable` なサブツリー（および常に text-input）だけが
    // ネイティブ選択に参加する（ADR-0097）。
    let user_select = resolve_user_select(kind, None);
    style.set_property("user-select", user_select)?;
    style.set_property("-webkit-user-select", user_select)?;
    match kind {
        ElementKind::ScrollView => {
            style.set_property("overflow", "auto")?;
            style.set_property("display", "flex")?;
            style.set_property("flex-direction", "column")?;
        }
        ElementKind::Image => {
            style.set_property("display", "block")?;
            style.set_property("object-fit", "fill")?;
        }
        ElementKind::TextInput => {
            // ブラウザのネイティブフォーカスリング（`:focus-visible`）を残す。
            // ここで `outline` を抑制すると「ブラウザが視覚基準」（ADR-0102）に反し、
            // DOM Renderer と乖離した。他の input 正規化（透明背景、font/color の
            // 継承）はそのまま。
            style.set_property("background", "transparent")?;
            style.set_property("font", "inherit")?;
            style.set_property("color", "inherit")?;
        }
        ElementKind::Button => {
            style.set_property("background", "transparent")?;
            style.set_property("font", "inherit")?;
            style.set_property("color", "inherit")?;
        }
        _ => {}
    }
    // 仕様の単一ソース由来の要素種別ごとの UA 既定カーソル（ADR-0105）:
    // button → pointer、text-input → text。Canvas（コアの `resolve_cursor`）と
    // 共有するため DOM と Canvas で同じカーソルになり、ここで再宣言しない。
    // `element_set_style` による明示 `cursor` は依然として優先される。
    let cursor = kind.default_cursor();
    if cursor != hayate_core::CursorValue::Default {
        let mut entries: Vec<(String, String)> = Vec::new();
        crate::generated::style_prop_css_entries(
            &hayate_core::StyleProp::Cursor(cursor),
            &mut entries,
        );
        if let Some((_, value)) = entries.into_iter().next() {
            style.set_property("cursor", &value)?;
        }
    }
    Ok(())
}

/// ページ読み込みごとに1回注入するドキュメントレベルの CSS ベースライン。
///
/// 冪等にするため `<style id="hayate-reset">` をセンチネルに使う。
/// グローバルルールが要素ごとのオーバーヘッドなしにドキュメント内の全要素
/// （Canvas モードのモックが作る隠し DOM ツリーも含む）を網羅する。
fn inject_baseline_stylesheet() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let doc = window.document().ok_or("no document")?;
    if doc.get_element_by_id("hayate-reset").is_some() {
        return Ok(());
    }
    let head = doc.head().ok_or("no head")?;
    let style_el = doc.create_element("style")?;
    style_el.set_id("hayate-reset");
    style_el.set_text_content(Some(
        "*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; } \
         html { font-size: 16px; line-height: 1; -webkit-text-size-adjust: 100%; } \
         body { font-size: inherit; line-height: inherit; } \
         img, canvas, svg, video { display: block; } \
         canvas { cursor: default; } \
         input, button, select, textarea { font: inherit; color: inherit; appearance: none; }",
    ));
    head.append_child(style_el.as_ref())?;
    Ok(())
}

/// CSS `@font-face` ルールをドキュメントへ注入し、ブラウザが
/// `font-family: <family_name>` でテキストを描画できるようにする。フォント
/// バイト列はデータ URL として渡す（HTML Mode が対象とするデモ・開発用途には十分）。
fn inject_font_face(family: &str, data: &[u8]) -> Result<(), JsValue> {
    use js_sys::Uint8Array;
    // 生バイトから組み立てたバイナリ文字列を btoa で base64 エンコードする。
    let bin: String = data.iter().map(|&b| b as char).collect();
    let window = web_sys::window().ok_or("no window")?;
    let b64 = window.btoa(&bin)?;
    let css =
        format!("@font-face {{ font-family: '{family}'; src: url(data:font/ttf;base64,{b64}); }}");
    let doc = window.document().ok_or("no document")?;
    let head = doc.head().ok_or("no head")?;
    let style_el = doc.create_element("style")?;
    style_el.set_text_content(Some(&css));
    head.append_child(style_el.as_ref())?;
    // Uint8Array が未使用であることを `_` で明示する。FontFace API への
    // 切り替え時に import を残せるようにしておく。
    let _ = Uint8Array::new_with_length(0);
    Ok(())
}
