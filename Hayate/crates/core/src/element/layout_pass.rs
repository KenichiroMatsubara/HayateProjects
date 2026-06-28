use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use taffy::{AvailableSpace, Dimension as TaffyDim, Size as TaffySize};

use crate::element::font_fetch::FontFetchTracker;
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::style::{StyleProp, ViewportCondition};
use crate::element::taffy_bridge::{self, MeasureCtx};
use crate::element::taffy_projection::{TaffyProjection, TraversalStep};
use crate::element::text::{self, TextLayout};
use crate::element::text_shaper::TextShaper;
use crate::element::tree::{Element, Event};

/// base の `layout_style`（作者の意図）に、現ビューポートで一致する **レイアウト系**
/// ビューポートバリアントを宣言順（後勝ち）で重ねた実効 Taffy スタイルを返す（ADR-0081）。
///
/// `apply_to_style` はレイアウト系プロップだけを適用しビジュアル系では何もしない（`false`
/// を返す）ので、`display:none` / `flex-direction` / `width` などの variant がレイアウトへ
/// 効くようになる。ビジュアル系 variant は従来どおり `resolve_effective` 側で解決する。
pub(crate) fn effective_layout_style(
    base: &taffy::Style,
    variants: &[(ViewportCondition, StyleProp)],
    viewport: (f32, f32),
) -> taffy::Style {
    let mut style = base.clone();
    for (condition, prop) in variants {
        if condition.matches(viewport.0, viewport.1) {
            taffy_bridge::apply_to_style(&mut style, prop);
        }
    }
    style
}

/// レイアウト計算とテキスト整形の状態をまとめる。`settle` 1 回で Taffy レイアウト、
/// Parley 整形、フォント dirty 伝播、FetchFont イベント発行、レイアウトキャッシュ更新を駆動する。
pub struct LayoutPass {
    pub(crate) projection: TaffyProjection,
    /// テキスト整形器（ADR-0123）。font collection（`FontContext`）と `LayoutContext` を
    /// 単独所有し、全シェイプ経路の唯一の入口になる。`projection`（箱）と対をなす。
    pub(crate) shaper: TextShaper,
    /// オンデマンドのフォント取得状態。`FetchFont` の重複発行を抑制し、
    /// 失敗した family は再試行、有限のリトライ予算を超えたら断念する。
    pub(crate) font_fetches: FontFetchTracker,
    /// 直近のカーソル可視トグルの実時刻ミリ秒（ADR-0032）。
    pub(crate) last_cursor_toggle_ms: Option<f64>,
    /// 要素ごとの絶対バウンディング矩形 (x, y, w, h)。`settle` ごとに更新し、
    /// 参照は `geometry` / `has_geometry` 経由のみ。
    layout_cache: HashMap<ElementId, (f32, f32, f32, f32)>,
}

impl LayoutPass {
    pub fn new() -> Self {
        Self {
            projection: TaffyProjection::new(),
            shaper: TextShaper::new(),
            font_fetches: FontFetchTracker::new(),
            last_cursor_toggle_ms: None,
            layout_cache: HashMap::new(),
        }
    }

    /// font collection への family 登録を整形器へ委譲する（`ElementTree::register_font`）。
    pub(crate) fn register_font(&mut self, family_name: &str, bytes: Arc<Vec<u8>>) {
        self.shaper.register_font(family_name, bytes);
    }

    /// 埋め込み family 名でフォントバイトを登録する（整形器へ委譲）。
    pub(crate) fn register_font_bytes(&mut self, bytes: Vec<u8>) {
        self.shaper.register_font_bytes(bytes);
    }

    /// toolbar 等の単発ラベルを整形する（整形器へ委譲、ADR-0097）。
    pub(crate) fn shape_label(&mut self, text: &str, font_size: f32) -> TextLayout {
        self.shaper.shape_label(text, font_size)
    }

    /// レイアウト用 `StyleProp` を `layout_style`（ドキュメントツリーが所有）へ変換し、
    /// 派生 Taffy ノードへ反映して layout-dirty を立てる。レイアウト以外の prop には
    /// `false` を返し、呼び出し側が Visual へ振り分ける。
    pub(crate) fn set_layout_prop(
        &mut self,
        id: ElementId,
        layout_style: &mut taffy::Style,
        prop: &StyleProp,
    ) -> bool {
        if !taffy_bridge::apply_to_style(layout_style, prop) {
            return false;
        }
        self.projection.set_style(id, layout_style.clone());
        true
    }

    /// 二面性を持つ `overflow` prop のレイアウト側。`overflow` を `layout_style` へ書き、
    /// Taffy ノードを再導出して layout-dirty を立てる。視覚側（子クリップ）は呼び出し側が
    /// Visual へ適用する。`overflow` は無効化のため visual prop として振り分けられるが、
    /// `overflow: hidden` / scroll-view が兄弟をはみ出さず縮むための flex スクロールコンテナ
    /// 最小サイズというレイアウト効果も持つため `set_layout_prop` とは別にする。
    pub(crate) fn set_overflow(
        &mut self,
        id: ElementId,
        layout_style: &mut taffy::Style,
        v: crate::element::style::OverflowValue,
    ) {
        taffy_bridge::apply_overflow_to_style(layout_style, v);
        self.projection.set_style(id, layout_style.clone());
    }

    /// 派生 Taffy プロジェクションを所有元の要素ツリーへ突き合わせ、Taffy レイアウト＋
    /// Parley 整形を実行し、絶対ジオメトリキャッシュを更新して、このパスでボックスの
    /// ジオメトリが変化（または新規出現）した要素集合を返す。
    ///
    /// 返すジオメトリ差分により、再フローしただけで他は clean なボックスも、古いジオメトリを
    /// 描かず再 lowering される。`structure_dirty` / `shape_dirty` / `fonts_dirty` は
    /// `ElementEngine` が所有し（ADR-0075）、本メソッドが消化する。
    pub(crate) fn settle(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
        structure_dirty: &mut HashSet<ElementId>,
        shape_dirty: &mut HashSet<ElementId>,
        fonts_dirty: &mut bool,
    ) -> HashSet<ElementId> {
        self.projection.reconcile(&*elements, root, structure_dirty);
        self.compute(elements, root, viewport, event_queue, shape_dirty, fonts_dirty);
        // 再構築前に旧ジオメトリをスナップショットして差分を取る。ボックス `(x, y, w, h)` が
        // 移動・リサイズ（または新規出現）した要素が返す集合に入る。挿入や選択による flex 再フローは
        // 自身は structure/visual dirty にならない祖先・兄弟へ波及するが、絶対座標なので移動した子孫が
        // それぞれ独立に差分へ入る。よって id 単位の再 lowering で十分。
        let previous = std::mem::take(&mut self.layout_cache);
        cache_layout(elements, &self.projection, root, 0.0, 0.0, &mut self.layout_cache);
        let mut geometry_dirty = HashSet::new();
        for (&id, geometry) in &self.layout_cache {
            if previous.get(&id) != Some(geometry) {
                geometry_dirty.insert(id);
            }
        }
        geometry_dirty
    }

    /// テスト用シーム（ADR-0042）。WASM ランタイムを模してフォントコレクションを再構築する。
    /// system_fonts なし、`default_font` をデフォルト family ＋ sans-serif generic として登録する。
    /// ホスト導入フォントに依存せず `.notdef → FetchFont → register_font` の実経路をテストできる。
    pub(crate) fn set_wasm_like_font_context(&mut self, default_font: Vec<u8>) {
        self.shaper.set_wasm_like_font_context(default_font);
        self.font_fetches = FontFetchTracker::new();
    }

    /// 直近の `settle` で得た絶対ボックス矩形 `(x, y, w, h)` を返す。ボックスジオメトリを
    /// 持たない要素（インラインテキスト等）は `None`。
    pub(crate) fn geometry(&self, id: ElementId) -> Option<(f32, f32, f32, f32)> {
        self.layout_cache.get(&id).copied()
    }

    /// `settle` が少なくとも 1 回ボックスジオメトリを生成済みなら true。
    pub(crate) fn has_geometry(&self) -> bool {
        !self.layout_cache.is_empty()
    }

    /// フォーカス要素のカーソルを 500ms ごとにトグルする（ADR-0032）。
    /// フォーカスが無い、または間隔未経過なら no-op。
    pub(crate) fn advance_cursor_blink(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        focused_element: Option<ElementId>,
        timestamp_ms: f64,
    ) -> Option<ElementId> {
        let focused = match focused_element {
            Some(id) => id,
            None => return None,
        };
        match self.last_cursor_toggle_ms {
            None => {
                // フォーカス直後の最初のフレーム。カーソルを表示しクロックを開始する。
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = elements.get_mut(&focused) {
                    el.cursor_visible = true;
                }
                Some(focused)
            }
            Some(prev) if timestamp_ms - prev >= 500.0 => {
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = elements.get_mut(&focused) {
                    el.cursor_visible = !el.cursor_visible;
                }
                Some(focused)
            }
            _ => None,
        }
    }

    /// `shape_dirty`/`fonts_dirty` を解決し、Taffy レイアウト＋Parley 整形を実行する。
    /// `shape_dirty`/`fonts_dirty` は `ElementEngine` が所有する（ADR-0075）。
    fn compute(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
        shape_dirty: &mut HashSet<ElementId>,
        fonts_dirty: &mut bool,
    ) {
        // 新フォント登録時は全テキストレイアウトを無効化し、このパスで新フォントデータで再整形する。
        for &id in shape_dirty.iter() {
            if let Some(el) = elements.get_mut(&id) {
                el.text_layout = None;
            }
        }

        if *fonts_dirty {
            *fonts_dirty = false;
            let text_ids: Vec<ElementId> = elements
                .iter()
                .filter_map(|(id, el)| {
                    if el.kind.is_text_like() {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for id in text_ids {
                if let Some(el) = elements.get_mut(&id) {
                    el.text_layout = None;
                    el.content_layout = None;
                    self.projection.mark_dirty(id);
                }
            }
        }

        let root_taffy = match self.projection.node_id(root) {
            Some(n) => n,
            None => return,
        };
        let root_source_size = elements[&root].layout_style.size;

        // ルートが Auto/Percent を指定した場合、寸法をビューポートにピン留めする。
        // ルートには包含ブロックが無く、これが無いと Percent は min-content に潰れる。
        // 現在の Taffy style ではなく layout_style（作者の意図）を使う。最初のピン留め後は
        // Taffy ノードが definite な Length を持つが、リサイズ時もビューポート変化に追従する必要がある。
        // ルートに明示された px Length 値はそのまま残す。
        if let Ok(mut style) = self.projection.taffy.style(root_taffy).cloned() {
            let mut changed = false;
            if !matches!(root_source_size.width, TaffyDim::Length(_)) {
                let pinned = TaffyDim::Length(viewport.0);
                if style.size.width != pinned {
                    style.size.width = pinned;
                    changed = true;
                }
            }
            if !matches!(root_source_size.height, TaffyDim::Length(_)) {
                let pinned = TaffyDim::Length(viewport.1);
                if style.size.height != pinned {
                    style.size.height = pinned;
                    changed = true;
                }
            }
            if changed {
                let _ = self.projection.taffy.set_style(root_taffy, style);
            }
        }

        let available = TaffySize {
            width: AvailableSpace::Definite(viewport.0),
            height: AvailableSpace::Definite(viewport.1),
        };

        let LayoutPass {
            projection,
            shaper,
            font_fetches,
            ..
        } = self;

        // 整形器に新しいレイアウトパスの開始を知らせ、幅キーのシェイプメモをクリアする。
        shaper.begin_layout();

        // measure を整形器へ配線する。IFC テキストは整形器の measure がシェイプしてメモを埋め、
        // `text-input` の UA デフォルト幅（ADR-0109）はフィールド自身のテキストに依存しない
        // フォント相対の固有コンテンツ幅を返す。明示 `width` / `flex-grow` / stretch は Taffy の
        // 固有解決でこれより優先される。
        {
            let taffy = &mut projection.taffy;
            let _ = taffy.compute_layout_with_measure(
                root_taffy,
                available,
                |known_dims, available_space, _node_id, ctx, _style| {
                    if let Some(MeasureCtx::TextInput(eid)) = ctx {
                        let eid = *eid;
                        let (font_size, font_weight, font_style, font_family) = {
                            let el = match elements.get(&eid) {
                                Some(e) => e,
                                None => return TaffySize::ZERO,
                            };
                            let ambient =
                                crate::element::ambient_defaults::ambient_at(elements, eid);
                            (
                                el.visual.font_size.unwrap_or(ambient.font_size),
                                el.visual.font_weight.or(ambient.font_weight),
                                el.visual.font_style,
                                el.visual.font_family.clone().or(ambient.font_family.clone()),
                            )
                        };
                        let width = shaper.text_input_default_width(
                            font_size,
                            font_family.as_deref(),
                            font_weight,
                            font_style,
                        );
                        return TaffySize {
                            width,
                            height: 0.0,
                        };
                    }
                    let eid = match ctx {
                        Some(MeasureCtx::Text(eid)) => *eid,
                        _ => return TaffySize::ZERO,
                    };
                    if elements.get(&eid).is_none() {
                        return TaffySize::ZERO;
                    }
                    let max_advance = match known_dims.width {
                        Some(w) => Some(w),
                        None => match available_space.width {
                            AvailableSpace::Definite(w) => Some(w),
                            AvailableSpace::MaxContent => None,
                            AvailableSpace::MinContent => Some(0.0),
                        },
                    };
                    let (width, height) = shaper.measure(elements, eid, max_advance, viewport);
                    TaffySize { width, height }
                },
            );
        }

        // 不変条件の機械的保証: 整形器が全 IFC テキストの retained グリフ層（`text_layout`）を
        // Taffy の確定（unrounded）ボックス幅で retain する。measure クロージャ last-wins と
        // compute 後の後付け再シェイプ（旧 baseline 行の min-content 折返し対策）はこの 1 点に畳まれた。
        // 欠落 family は値で返り、`FetchFont` 発行はここ 1 箇所で行う（IFC の発行サイトの集約）。
        let outcome = shaper.finalize_ifc(projection, elements, viewport);
        for family in outcome.missing_families {
            if font_fetches.should_request(&family) {
                font_fetches.mark_requested(&family);
                event_queue.push(Event::FetchFont { family });
            }
        }
        for eid in outcome.finalized {
            shape_dirty.remove(&eid);
        }

        // TextInput 要素のコンテンツレイアウトを構築する（Canvas モードの描画＋カーソル用）。
        let textinput_ids: Vec<ElementId> = elements
            .iter()
            .filter_map(|(id, el)| {
                if el.kind == ElementKind::TextInput {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        for eid in textinput_ids {
            let (display_text, font_size, font_weight, font_style) = {
                let el = match elements.get(&eid) {
                    Some(e) => e,
                    None => continue,
                };
                let ambient = crate::element::ambient_defaults::ambient_at(elements, eid);
                let text = el
                    .edit
                    .as_ref()
                    .map(|edit| edit.display_text())
                    .unwrap_or_default();
                (
                    text,
                    el.visual.font_size.unwrap_or(ambient.font_size),
                    el.visual.font_weight.or(ambient.font_weight),
                    el.visual.font_style,
                )
            };

            let (max_advance, font_family) = {
                let ambient = crate::element::ambient_defaults::ambient_at(elements, eid);
                let el = elements.get(&eid).map(|e| {
                    (
                        projection.node_id(eid).and_then(|n| {
                            projection
                                .taffy
                                .layout(n)
                                .ok()
                                .map(|l| l.content_box_width())
                        }),
                        e.visual
                            .font_family
                            .clone()
                            .or(ambient.font_family.clone()),
                    )
                });
                el.map(|(a, f)| (a, f)).unwrap_or((None, None))
            };

            let is_placeholder = display_text.is_empty();
            let text_to_layout: Option<String> = if is_placeholder {
                elements
                    .get(&eid)
                    .and_then(|el| el.text.clone())
                    .filter(|t| !t.is_empty())
            } else {
                Some(display_text)
            };

            if let Some(text) = text_to_layout {
                let layout = shaper.shape_text(
                    &text,
                    font_size,
                    max_advance,
                    font_family.as_deref(),
                    font_weight,
                    font_style,
                );

                for &fam in &layout.missing_families {
                    if font_fetches.should_request(fam) {
                        font_fetches.mark_requested(fam);
                        event_queue.push(Event::FetchFont {
                            family: fam.to_string(),
                        });
                    }
                }
                if let Some(ref fam) = font_family {
                    // `font-family` はスタック。カンマ区切り全体ではなく、名前付きエントリを
                    // 個別に解決・取得する。
                    for resolved in text::parse_font_family_list(fam) {
                        if resolved != text::DEFAULT_FONT_FAMILY
                            && font_fetches.should_request(resolved)
                            && !shaper.has_family(resolved)
                        {
                            font_fetches.mark_requested(resolved);
                            event_queue.push(Event::FetchFont {
                                family: resolved.to_string(),
                            });
                        }
                    }
                }
                if let Some(el) = elements.get_mut(&eid) {
                    if is_placeholder {
                        el.content_layout = None;
                        el.text_layout = Some(layout);
                    } else {
                        el.content_layout = Some(layout);
                        el.text_layout = None;
                        if let Some(edit) = el.edit.as_mut() {
                            // 新しく整形したコンテンツに対しキャレット/選択を有効に保つが、
                            // 末尾へ強制してはならない。これは毎回のリレイアウト（style 変更・
                            // リサイズ・選択起因の再描画）で走るため、カーソルを `len` へ
                            // 強制するとクリックで置いたばかりのキャレットを壊す（アンカーは
                            // 据え置きでカーソルだけ末尾へ飛び、クリック点から末尾文字までの
                            // 幻の選択を生む）。クランプはテキスト縮小後に範囲外となった
                            // オフセットの修復のみ行う。キャレット位置自体は edit/pointer 操作が
                            // 所有し、レイアウトパスは触らない。
                            let len = edit.text_content.len();
                            edit.cursor_byte_index = edit.cursor_byte_index.min(len);
                            edit.selection_anchor = edit.selection_anchor.min(len);
                        }
                    }
                }
            } else if let Some(el) = elements.get_mut(&eid) {
                el.content_layout = None;
                el.text_layout = None;
            }
        }
    }
}

impl Default for LayoutPass {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::kind::ElementKind;
    use crate::element::style::{Dimension, StyleProp};
    use crate::element::tree::Visual;

    fn view(parent: Option<ElementId>, children: Vec<ElementId>) -> Element {
        Element {
            kind: ElementKind::View,
            parent,
            children,
            layout_style: taffy::Style::default(),
            visual: Visual::default(),
            text: None,
            src: None,
            text_layout: None,
            transform: None,
            scroll_offset: (0.0, 0.0),
            src_image: None,
            edit: None,
            cursor_visible: false,
            content_layout: None,
            aria_label: None,
            role: None,
            pseudo_styles: Default::default(),
            disabled: false,
            selectable: false,
            user_select: crate::element::style::UserSelectValue::Text,
            multiline: false,
            viewport_variants: Vec::new(),
        }
    }

    /// 縮約レイアウトインターフェース。呼び出し側は layout prop を設定して settle し、
    /// ジオメトリを読む。bridge 変換・reconcile・compute・レイアウトキャッシュに直接触れない。
    #[test]
    fn set_layout_prop_then_settle_then_geometry_lays_out_child() {
        let mut layout = LayoutPass::new();
        let root_id = ElementId::from_u64(1);
        let child_id = ElementId::from_u64(2);
        let mut elements = HashMap::new();
        elements.insert(root_id, view(None, vec![child_id]));
        elements.insert(child_id, view(Some(root_id), Vec::new()));

        {
            let child = elements.get_mut(&child_id).unwrap();
            assert!(layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Width(Dimension::px(80.0)),
            ));
            assert!(layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Height(Dimension::px(40.0)),
            ));
        }

        let mut structure_dirty = HashSet::new();
        let mut shape_dirty = HashSet::new();
        let mut fonts_dirty = false;
        let mut events = Vec::new();
        layout.settle(
            &mut elements,
            root_id,
            (300.0, 200.0),
            &mut events,
            &mut structure_dirty,
            &mut shape_dirty,
            &mut fonts_dirty,
        );

        let rect = layout.geometry(child_id).expect("child must have geometry");
        assert!((rect.2 - 80.0).abs() < 0.5, "width was {}", rect.2);
        assert!((rect.3 - 40.0).abs() < 0.5, "height was {}", rect.3);
    }

    /// `settle` がジオメトリ差分（移動/リサイズ/出現したボックス）を返すので、
    /// 呼び出し側はレイアウトキャッシュ自体をスナップショット・比較する必要がない。
    #[test]
    fn settle_reports_geometry_diff_only_for_changed_boxes() {
        let mut layout = LayoutPass::new();
        let root_id = ElementId::from_u64(1);
        let child_id = ElementId::from_u64(2);
        let mut elements = HashMap::new();
        elements.insert(root_id, view(None, vec![child_id]));
        elements.insert(child_id, view(Some(root_id), Vec::new()));
        {
            let child = elements.get_mut(&child_id).unwrap();
            layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Width(Dimension::px(80.0)),
            );
            layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Height(Dimension::px(40.0)),
            );
        }

        let mut structure_dirty = HashSet::new();
        let mut shape_dirty = HashSet::new();
        let mut fonts_dirty = false;
        let mut events = Vec::new();
        let viewport = (300.0, 200.0);

        // 最初の settle ではすべてのボックスが新規として差分に現れる。
        let appeared = layout.settle(
            &mut elements, root_id, viewport, &mut events,
            &mut structure_dirty, &mut shape_dirty, &mut fonts_dirty,
        );
        assert!(appeared.contains(&child_id));

        // 変更なしで再 settle。安定レイアウトは空の差分を返す。
        let stable = layout.settle(
            &mut elements, root_id, viewport, &mut events,
            &mut structure_dirty, &mut shape_dirty, &mut fonts_dirty,
        );
        assert!(stable.is_empty(), "stable layout must report no geometry diff");

        // 縮約 set インターフェース経由でリサイズしてから settle する。
        {
            let child = elements.get_mut(&child_id).unwrap();
            layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Height(Dimension::px(90.0)),
            );
        }
        let resized = layout.settle(
            &mut elements, root_id, viewport, &mut events,
            &mut structure_dirty, &mut shape_dirty, &mut fonts_dirty,
        );
        assert!(resized.contains(&child_id), "resized box must be in geometry diff");
        let rect = layout.geometry(child_id).expect("child geometry");
        assert!((rect.3 - 90.0).abs() < 0.5, "height was {}", rect.3);
    }

    /// set インターフェースはレイアウト prop のみ変換する。visual prop は拒否され（false を返し）
    /// `layout_style` を変えないので、呼び出し側は Visual へ振り分けられる。
    #[test]
    fn set_layout_prop_rejects_non_layout_prop() {
        let mut layout = LayoutPass::new();
        let id = ElementId::from_u64(1);
        let mut style = taffy::Style::default();
        let before = style.clone();

        let applied = layout.set_layout_prop(id, &mut style, &StyleProp::Opacity(0.5));

        assert!(!applied, "visual prop must not be accepted by the layout seam");
        assert_eq!(style, before, "non-layout prop must not mutate layout_style");
    }
}

fn cache_layout(
    elements: &HashMap<ElementId, Element>,
    projection: &TaffyProjection,
    id: ElementId,
    ox: f32,
    oy: f32,
    cache: &mut HashMap<ElementId, (f32, f32, f32, f32)>,
) {
    match projection.traversal_step(elements, id) {
        // インラインテキスト要素はボックスジオメトリを持たないが、子孫はたどる。
        Some(TraversalStep::Skip(el)) => {
            for &child in &el.children {
                cache_layout(elements, projection, child, ox, oy, cache);
            }
        }
        Some(TraversalStep::Visit(taffy_node, el)) => {
            let layout = match projection.taffy.layout(taffy_node) {
                Ok(l) => l,
                Err(_) => return,
            };
            let x = ox + layout.location.x;
            let y = oy + layout.location.y;
            cache.insert(id, (x, y, layout.size.width, layout.size.height));
            for &child in &el.children {
                cache_layout(elements, projection, child, x, y, cache);
            }
        }
        None => {}
    }
}
