//! テキスト整形器（ADR-0123）。
//!
//! font collection（[`FontContext`]）と Parley の [`LayoutContext`] を**単独所有**し、
//! 全シェイプ経路（IFC・text-input content・UA default width・toolbar ラベル）の唯一の
//! 入口になる。`TaffyProjection`（箱）と対をなし、グリフを所有する deep module。
//! `ElementTree` の public surface は不変で、`LayoutPass` の private field として抱える
//! （ADR-0075 と同型の「内部 module 抽出・public 不変」方針）。

use std::collections::HashMap;
use std::sync::Arc;

use linebender_resource_handle::Blob;
use parley::{FontContext, LayoutContext};

use crate::element::ambient_defaults;
use crate::element::id::ElementId;
use crate::element::inline_text;
use crate::element::kind::ElementKind;
use crate::element::style::FontStyleValue;
use crate::element::taffy_projection::TaffyProjection;
use crate::element::text::{self, TextBrush, TextLayout};
use crate::element::tree::Element;

/// 幅キーのシェイプメモのマッチング許容幅（px）。Taffy の確定（unrounded）インナー幅は
/// 丸めや浮動小数差で measure 時に使った幅と僅かにずれ得るため、この許容内なら同一幅と
/// みなしメモを再利用する。ミスしても finalize がその場で box幅シェイプ（無メモ版）へ
/// 優雅に劣化するので、正しさには影響しない純粋な最適化のための tunable。
const SHAPE_MEMO_WIDTH_TOLERANCE_PX: f32 = 0.5;

/// settle ごとの幅キーのシェイプメモの 1 エントリ。
struct ShapeMemoEntry {
    /// このレイアウトを生成した `max_advance`（`None` = 無制約）。メモキーの幅成分。
    width: Option<f32>,
    layout: TextLayout,
}

/// 2 つのメモキー幅が許容内で一致するか（[`SHAPE_MEMO_WIDTH_TOLERANCE_PX`]）。
fn width_keys_match(a: Option<f32>, b: Option<f32>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() <= SHAPE_MEMO_WIDTH_TOLERANCE_PX,
        _ => false,
    }
}

/// `finalize` の戻り値。欠落フォント family（値で返し、`FetchFont` 発行は呼び出し側が 1 箇所で行う）
/// と、retain した text-input 集合。
pub(crate) struct FinalizeOutcome {
    /// 取得すべき欠落 family 名（notdef 検出 + 名前付き先読みで collection 未登録のもの）。
    /// IFC と text-input の両経路を 1 つに de-dup した単一集合。
    pub(crate) missing_families: Vec<String>,
    /// `content_layout` を retain した text-input（プレースホルダではなく編集テキスト）。
    /// キャレット/選択クランプ（編集意味論・ADR-0069）は整形器の外＝Layout Pass 後処理で行うため、
    /// 対象集合だけを返す。
    pub(crate) content_finalized: Vec<ElementId>,
}

/// font collection と `LayoutContext` を所有し、全テキストを整形する内部 module。
/// settle ごとの幅キーのシェイプメモを抱え、「retained グリフは最終ボックス幅で
/// シェイプされる」box幅不変条件を `finalize` が機械的に保証する。
pub(crate) struct TextShaper {
    font_cx: FontContext,
    layout_cx: LayoutContext<TextBrush>,
    /// settle ごとの幅キーのシェイプメモ。`(eid, 幅)` でメモ化し、`begin_layout` でクリアする。
    /// measure が埋め、finalize が box幅エントリを消化する。
    shape_memo: HashMap<ElementId, Vec<ShapeMemoEntry>>,
}

impl TextShaper {
    pub(crate) fn new() -> Self {
        let mut font_cx = FontContext::new();
        init_bundled_fonts(&mut font_cx);
        Self {
            font_cx,
            layout_cx: LayoutContext::new(),
            shape_memo: HashMap::new(),
        }
    }

    /// 生バイトから family を font collection に登録する（[`ElementTree::register_font`] が委譲）。
    /// 要求された名前で登録し、バンドル既定の後ろにクラスタ単位のフォールバックとして組み込む。
    ///
    /// [`ElementTree::register_font`]: crate::element::tree::ElementTree::register_font
    pub(crate) fn register_font(&mut self, family_name: &str, bytes: Arc<Vec<u8>>) {
        text::register_collection_font(&mut self.font_cx.collection, family_name, bytes);
    }

    /// フォントファイル自身に埋め込まれた family 名を使って生バイトから登録する。
    pub(crate) fn register_font_bytes(&mut self, bytes: Vec<u8>) {
        let blob = Blob::new(Arc::new(bytes));
        self.font_cx.collection.register_fonts(blob, None);
    }

    /// `family` が font collection に登録済みなら true。名前付き `font-family` の先読み
    /// 取得判定（未登録なら `FetchFont`）に使う。
    fn has_family(&mut self, family: &str) -> bool {
        self.font_cx.collection.family_id(family).is_some()
    }

    /// toolbar 等の単発ラベルを整形する（ADR-0097）。
    pub(crate) fn shape_label(&mut self, text: &str, font_size: f32) -> TextLayout {
        text::build_text_layout(
            &mut self.font_cx,
            &mut self.layout_cx,
            text,
            font_size,
            None,
            None,
            None,
            None,
        )
    }

    /// 新しいレイアウトパスの開始。settle ごとの幅キーのシェイプメモをクリアする。
    pub(crate) fn begin_layout(&mut self) {
        self.shape_memo.clear();
    }

    /// Taffy への寸法回答。IFC ルート `eid` を `max_advance` で整形し、幅キーのメモを埋め、
    /// レイアウトの `(width, height)` を返す。同じ `(eid, 幅)` が既にメモにあれば再シェイプしない。
    pub(crate) fn measure(
        &mut self,
        elements: &HashMap<ElementId, Element>,
        eid: ElementId,
        max_advance: Option<f32>,
        viewport: (f32, f32),
    ) -> (f32, f32) {
        if let Some(entries) = self.shape_memo.get(&eid) {
            if let Some(e) = entries
                .iter()
                .find(|e| width_keys_match(e.width, max_advance))
            {
                return layout_size(&e.layout);
            }
        }
        let layout = inline_text::shape(
            elements,
            eid,
            max_advance,
            &mut self.font_cx,
            &mut self.layout_cx,
            viewport,
        );
        let size = layout_size(&layout);
        self.shape_memo
            .entry(eid)
            .or_default()
            .push(ShapeMemoEntry {
                width: max_advance,
                layout,
            });
        size
    }

    /// box幅不変条件の機械的保証。**両 retained 層**（IFC の `text_layout` と text-input の
    /// `content_layout`）を Taffy の確定（unrounded）ボックス幅で retain し、Text Shaper が所有する。
    ///
    /// - IFC: measure 済みの全 IFC ルートについて、幅キーのメモに box幅一致エントリがあれば再利用し
    ///   （通常ケース＝measure が既に box幅で shape 済み）、無ければその場で box幅シェイプ
    ///   （無メモ劣化＝決して間違わない）。
    /// - text-input: 編集テキスト/プレースホルダを同じ確定ボックス幅で整形し `content_layout`
    ///   （プレースホルダは `text_layout`）へ retain する。別ループは解消。
    ///
    /// 欠落 family の検出（notdef + 名前付き先読み）は両経路を 1 つに de-dup した単一集合として
    /// **値で返す**（`FetchFont` 発行は呼び出し側が 1 箇所で行う）。キャレット/選択クランプ
    /// （編集意味論・ADR-0069）は整形器の対象外で、`content_finalized` の集合を返すのみ。
    pub(crate) fn finalize(
        &mut self,
        projection: &TaffyProjection,
        elements: &mut HashMap<ElementId, Element>,
        viewport: (f32, f32),
    ) -> FinalizeOutcome {
        let mut missing_families: Vec<String> = Vec::new();
        let mut content_finalized: Vec<ElementId> = Vec::new();

        // --- IFC の retained グリフ層（text_layout）---
        let ifc_eids: Vec<ElementId> = self.shape_memo.keys().copied().collect();
        for eid in ifc_eids {
            let layout = match unrounded_inner_width(projection, eid) {
                Some(w) if w.is_finite() && w > 0.0 => self.take_or_shape(elements, eid, w, viewport),
                // box幅不明: measure 済みの last-wins レイアウトを使う（reshape しない）。
                _ => self.take_last(eid),
            };
            let Some(mut layout) = layout else { continue };
            if layout.text.is_empty() {
                continue;
            }
            // HTML モードが DOM テキストノードへ戻せるよう、各 lowered run に元テキストを刻み直す。
            restamp_run_text(&mut layout);
            let named = elements.get(&eid).and_then(|el| el.visual.font_family.clone());
            self.collect_missing_into(&layout, named.as_deref(), &mut missing_families);
            if let Some(el) = elements.get_mut(&eid) {
                el.text_layout = Some(layout);
            }
        }
        self.shape_memo.clear();

        // --- text-input の retained コンテンツ層（content_layout / プレースホルダ text_layout）---
        let textinput_eids: Vec<ElementId> = elements
            .iter()
            .filter_map(|(id, el)| (el.kind == ElementKind::TextInput).then_some(*id))
            .collect();
        for eid in textinput_eids {
            let ambient = ambient_defaults::ambient_at(elements, eid);
            let Some(el) = elements.get(&eid) else { continue };
            let display_text = el
                .edit
                .as_ref()
                .map(|edit| edit.display_text())
                .unwrap_or_default();
            let font_size = el.visual.font_size.unwrap_or(ambient.font_size);
            let font_weight = el.visual.font_weight.or(ambient.font_weight);
            let font_style = el.visual.font_style;
            let font_family = el.visual.font_family.clone().or(ambient.font_family.clone());
            let placeholder = el.text.clone();

            // 確定（unrounded）ボックス幅。IFC と同じソースで box幅を 1 箇所に決める。
            let max_advance = unrounded_inner_width(projection, eid).filter(|w| w.is_finite());

            let is_placeholder = display_text.is_empty();
            let text_to_layout: Option<String> = if is_placeholder {
                placeholder.filter(|t| !t.is_empty())
            } else {
                Some(display_text)
            };

            if let Some(text) = text_to_layout {
                let layout = self.shape_text(
                    &text,
                    font_size,
                    max_advance,
                    font_family.as_deref(),
                    font_weight,
                    font_style,
                );
                self.collect_missing_into(&layout, font_family.as_deref(), &mut missing_families);
                if let Some(el) = elements.get_mut(&eid) {
                    if is_placeholder {
                        el.content_layout = None;
                        el.text_layout = Some(layout);
                    } else {
                        el.content_layout = Some(layout);
                        el.text_layout = None;
                        content_finalized.push(eid);
                    }
                }
            } else if let Some(el) = elements.get_mut(&eid) {
                el.content_layout = None;
                el.text_layout = None;
            }
        }

        FinalizeOutcome {
            missing_families,
            content_finalized,
        }
    }

    /// 欠落 family を `out` に集める: notdef 検出（`layout.missing_families`）に加え、collection
    /// 未登録の名前付き `font-family` を先読み取得対象として載せる（Latin フォントは .notdef を
    /// 生まず script 検出が発火しないため）。`font-family` スタックはエントリ単位で解決する。
    fn collect_missing_into(
        &mut self,
        layout: &TextLayout,
        font_family: Option<&str>,
        out: &mut Vec<String>,
    ) {
        for &fam in &layout.missing_families {
            out.push(fam.to_string());
        }
        if let Some(fam) = font_family {
            let resolved: Vec<String> = text::parse_font_family_list(fam)
                .iter()
                .map(|s| s.to_string())
                .collect();
            for r in resolved {
                if r != text::DEFAULT_FONT_FAMILY && !self.has_family(&r) {
                    out.push(r);
                }
            }
        }
    }

    /// `eid` の幅 `width` のメモエントリがあれば取り出し、無ければその場で box幅シェイプする。
    fn take_or_shape(
        &mut self,
        elements: &HashMap<ElementId, Element>,
        eid: ElementId,
        width: f32,
        viewport: (f32, f32),
    ) -> Option<TextLayout> {
        if let Some(entries) = self.shape_memo.get_mut(&eid) {
            if let Some(idx) = entries
                .iter()
                .position(|e| width_keys_match(e.width, Some(width)))
            {
                return Some(entries.remove(idx).layout);
            }
        }
        Some(inline_text::shape(
            elements,
            eid,
            Some(width),
            &mut self.font_cx,
            &mut self.layout_cx,
            viewport,
        ))
    }

    /// `eid` の最後に measure したレイアウト（last-wins）を取り出す。
    fn take_last(&mut self, eid: ElementId) -> Option<TextLayout> {
        self.shape_memo
            .get_mut(&eid)
            .and_then(|v| v.pop())
            .map(|e| e.layout)
    }

    /// 単一テキスト（text-input のコンテンツ/プレースホルダ）を整形する。
    #[allow(clippy::too_many_arguments)]
    fn shape_text(
        &mut self,
        text: &str,
        font_size: f32,
        max_advance: Option<f32>,
        font_family: Option<&str>,
        font_weight: Option<f32>,
        font_style: Option<FontStyleValue>,
    ) -> TextLayout {
        text::build_text_layout(
            &mut self.font_cx,
            &mut self.layout_cx,
            text,
            font_size,
            max_advance,
            font_family,
            font_weight,
            font_style,
        )
    }

    /// 幅未指定 `text-input` の UA 既定コンテンツ幅（ADR-0109）。フィールド自身のテキストに
    /// 依存しない、フォント相対の固有コンテンツ幅。
    pub(crate) fn text_input_default_width(
        &mut self,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<f32>,
        font_style: Option<FontStyleValue>,
    ) -> f32 {
        text::text_input_default_width(
            &mut self.font_cx,
            &mut self.layout_cx,
            font_size,
            font_family,
            font_weight,
            font_style,
        )
    }

    /// テスト用シーム（ADR-0042）。WASM ランタイムを模してフォントコレクションを再構築する。
    /// system_fonts なし、`default_font` をデフォルト family ＋ sans-serif generic として登録する。
    /// ホスト導入フォントに依存せず `.notdef → FetchFont → register_font` の実経路をテストできる。
    pub(crate) fn set_wasm_like_font_context(&mut self, default_font: Vec<u8>) {
        use fontique::{Collection, CollectionOptions, FontInfoOverride, GenericFamily};
        self.font_cx.collection = Collection::new(CollectionOptions {
            system_fonts: false,
            ..Default::default()
        });
        let blob = Blob::new(Arc::new(default_font));
        let override_info = FontInfoOverride {
            family_name: Some(text::DEFAULT_FONT_FAMILY),
            ..Default::default()
        };
        let registered = self.font_cx.collection.register_fonts(blob, Some(override_info));
        let ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
        if !ids.is_empty() {
            self.font_cx
                .collection
                .set_generic_families(GenericFamily::SansSerif, ids.into_iter());
        }
    }
}

impl Default for TextShaper {
    fn default() -> Self {
        Self::new()
    }
}

/// measure が Taffy へ返す寸法。空テキストは（一行分の高さを持ち得るが）ボックスを生まない
/// ので `(0, 0)` に潰す（旧 measure クロージャの early-return ZERO と同値）。
fn layout_size(layout: &TextLayout) -> (f32, f32) {
    if layout.text.is_empty() {
        (0.0, 0.0)
    } else {
        (layout.layout.width(), layout.layout.height())
    }
}

/// `eid` の確定（unrounded）インナーボックス幅。Taffy は整数ピクセルへ丸めるため、丸め前の
/// 幅で揃えればグリフ折返しがレイアウトの決めた幅と厳密に一致する。両 retained 層（IFC・
/// text-input）でこの 1 つの式から box幅を導き、丸め/未丸めソースを 1 箇所に決める。
fn unrounded_inner_width(projection: &TaffyProjection, eid: ElementId) -> Option<f32> {
    projection.node_id(eid).map(|node| {
        let l = projection.taffy.unrounded_layout(node);
        l.size.width - l.padding.left - l.padding.right - l.border.left - l.border.right
    })
}

/// 各 lowered run に元テキストを刻み直す（HTML モードの DOM テキストノード復元用）。
fn restamp_run_text(layout: &mut TextLayout) {
    let src: Arc<str> = layout.text.clone();
    for run in &mut layout.runs {
        if let Some(rd) = Arc::get_mut(run) {
            rd.text = src.clone();
        }
    }
}

fn init_bundled_fonts(font_cx: &mut FontContext) {
    use fontique::{FontInfoOverride, GenericFamily};

    static NOTO_SANS_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSansJP.ttf");

    let blob = Blob::new(Arc::new(NOTO_SANS_BYTES));
    let override_info = FontInfoOverride {
        family_name: Some(text::DEFAULT_FONT_FAMILY),
        ..Default::default()
    };
    let registered = font_cx.collection.register_fonts(blob, Some(override_info));
    let family_ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
    if !family_ids.is_empty() {
        font_cx
            .collection
            .set_generic_families(GenericFamily::SansSerif, family_ids.into_iter());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use taffy::{AvailableSpace, Dimension as TaffyDim, Size as TaffySize, Style as TaffyStyle};

    use super::*;
    use crate::element::kind::ElementKind;
    use crate::element::taffy_bridge::MeasureCtx;
    use crate::element::tree::Visual;

    const VIEWPORT: (f32, f32) = (800.0, 600.0);

    fn base_element(kind: ElementKind, parent: Option<ElementId>) -> Element {
        Element {
            kind,
            parent,
            children: Vec::new(),
            layout_style: TaffyStyle::default(),
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

    /// 幅 `box_w` の column コンテナ root に、stretch で box 幅へ広がる単一 IFC テキスト子を置く。
    /// stretch によりテキストノードの確定幅が `box_w` になるので、finalize の box幅 retain を
    /// 観測できる（狭ければ折り返す）。
    fn one_text_in_box(
        text: &str,
        font_family: Option<&str>,
        box_w: f32,
    ) -> (HashMap<ElementId, Element>, ElementId, ElementId) {
        let root_id = ElementId::from_u64(1);
        let text_id = ElementId::from_u64(2);

        let mut root = base_element(ElementKind::View, None);
        root.children = vec![text_id];
        root.layout_style = TaffyStyle {
            flex_direction: taffy::FlexDirection::Column,
            size: TaffySize {
                width: TaffyDim::length(box_w),
                height: TaffyDim::auto(),
            },
            ..Default::default()
        };

        let mut text_el = base_element(ElementKind::Text, Some(root_id));
        text_el.text = Some(text.to_string());
        text_el.visual.font_size = Some(16.0);
        text_el.visual.font_family = font_family.map(|s| s.to_string());

        let mut elements = HashMap::new();
        elements.insert(root_id, root);
        elements.insert(text_id, text_el);
        (elements, root_id, text_id)
    }

    /// テキストレイアウトのために最小の Taffy projection を組み、measure を整形器へ配線して
    /// compute し、`finalize_ifc` を呼ぶ。retained `text_layout` を要素へ書き込み、outcome を返す。
    fn lay_out(
        shaper: &mut TextShaper,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
    ) -> FinalizeOutcome {
        let mut projection = TaffyProjection::new();
        projection.set_layout_viewport(VIEWPORT);
        let mut structure_dirty: HashSet<ElementId> = HashSet::new();
        projection.reconcile(&*elements, root, &mut structure_dirty);

        let root_node = projection.node_id(root).expect("root projected node");
        let available = TaffySize {
            width: AvailableSpace::Definite(VIEWPORT.0),
            height: AvailableSpace::Definite(VIEWPORT.1),
        };

        shaper.begin_layout();
        {
            let taffy = &mut projection.taffy;
            let _ = taffy.compute_layout_with_measure(
                root_node,
                available,
                |known_dims, available_space, _node, ctx, _style| {
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
                    let (width, height) = shaper.measure(elements, eid, max_advance, VIEWPORT);
                    TaffySize { width, height }
                },
            );
        }
        shaper.finalize(&projection, elements, VIEWPORT)
    }

    /// 整形器の `measure` は幅キーでメモ化する: 同一 `(eid, 幅)` の再 measure は再シェイプせず
    /// 同じ寸法を返し（決定性）、メモのエントリは増えない。異なる幅は別エントリになる。
    #[test]
    fn measure_is_memoized_per_width_key_and_deterministic() {
        let mut shaper = TextShaper::new();
        let (elements, _root, text_id) = one_text_in_box("hello world foo bar", None, 200.0);

        shaper.begin_layout();
        let first = shaper.measure(&elements, text_id, Some(200.0), VIEWPORT);
        let again = shaper.measure(&elements, text_id, Some(200.0), VIEWPORT);
        assert_eq!(first, again, "same (eid, width) must return identical size");
        assert_eq!(
            shaper.shape_memo.get(&text_id).map(|v| v.len()),
            Some(1),
            "re-measuring the same width key must not append a second memo entry"
        );

        // 許容幅内（< 0.5px）の幅も同一キーとみなし再シェイプしない。
        let near = shaper.measure(&elements, text_id, Some(200.3), VIEWPORT);
        assert_eq!(first, near, "widths within tolerance share the memo entry");
        assert_eq!(shaper.shape_memo.get(&text_id).map(|v| v.len()), Some(1));

        // はっきり異なる幅は別エントリ。
        let _ = shaper.measure(&elements, text_id, Some(60.0), VIEWPORT);
        assert_eq!(
            shaper.shape_memo.get(&text_id).map(|v| v.len()),
            Some(2),
            "a distinct width key must create a new memo entry"
        );
    }

    /// `finalize_ifc` は retained `text_layout` を projection の確定ボックス幅でシェイプする。
    /// 狭い箱では折り返して複数行になり、レイアウト幅は箱幅を超えない。広い箱では 1 行。
    #[test]
    fn finalize_retains_glyphs_at_projection_box_width() {
        let text = "hello world foo bar baz qux";

        // 狭い箱（60px）: 折り返して複数行、幅は箱に収まる。
        let mut shaper = TextShaper::new();
        let (mut narrow, root, text_id) = one_text_in_box(text, None, 60.0);
        let _ = lay_out(&mut shaper, &mut narrow, root);
        let tl = narrow[&text_id]
            .text_layout
            .as_ref()
            .expect("finalize must retain text_layout");
        let narrow_lines = tl.layout.lines().count();
        assert!(
            narrow_lines > 1,
            "narrow box must wrap to multiple lines, got {narrow_lines}"
        );
        assert!(
            tl.layout.width() <= 60.0 + SHAPE_MEMO_WIDTH_TOLERANCE_PX,
            "retained glyphs must wrap within the box width, got {}",
            tl.layout.width()
        );

        // 広い箱（600px）: 同じテキストが 1 行に収まる。box幅で retain される証左。
        let mut shaper = TextShaper::new();
        let (mut wide, root, text_id) = one_text_in_box(text, None, 600.0);
        let _ = lay_out(&mut shaper, &mut wide, root);
        let wide_lines = wide[&text_id]
            .text_layout
            .as_ref()
            .expect("finalize must retain text_layout")
            .layout
            .lines()
            .count();
        assert_eq!(wide_lines, 1, "wide box must keep the text on one line");
    }

    /// 同一入力での `finalize` は決定的: 行数が再現する。
    #[test]
    fn finalize_is_deterministic_across_runs() {
        let text = "hello world foo bar baz qux";
        let lines = || -> usize {
            let mut shaper = TextShaper::new();
            let (mut elements, root, text_id) = one_text_in_box(text, None, 60.0);
            let _ = lay_out(&mut shaper, &mut elements, root);
            elements[&text_id]
                .text_layout
                .as_ref()
                .unwrap()
                .layout
                .lines()
                .count()
        };
        assert_eq!(lines(), lines(), "finalize line count must be deterministic");
    }

    /// `finalize_ifc` は欠落 family を値で返す: collection 未登録の名前付き `font-family` は
    /// 先読み取得対象として戻り集合に載る（`FetchFont` 発行は呼び出し側の 1 箇所が行う）。
    #[test]
    fn finalize_returns_missing_named_family() {
        let mut shaper = TextShaper::new();
        let (mut elements, root, _text_id) = one_text_in_box("hello", Some("Inter"), 600.0);
        let outcome = lay_out(&mut shaper, &mut elements, root);
        assert!(
            outcome
                .missing_families
                .iter()
                .any(|f| f == "Inter"),
            "an unregistered named family must be returned as missing, got {:?}",
            outcome.missing_families
        );
    }
}
