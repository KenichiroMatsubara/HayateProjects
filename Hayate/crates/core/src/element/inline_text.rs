use std::collections::HashMap;

use parley::{FontContext, LayoutContext};

use crate::color::Color;
use crate::element::ambient_defaults;
use crate::element::effective_visual::{
    self, child_inherited_context, InheritedVisualContext,
};
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::pseudo_state::InteractionSnapshot;
use crate::element::style::{FontStyleValue, TextDecorationValue, TextOverflowValue};
use crate::element::text::{
    self, RangeMap, TextBrush, TextLayout, build_ranged_text_layout, RangedTextSpan,
};
use crate::element::tree::{Element, Visual};

/// 親も `text` である `text` 要素 — Taffy ボックスを持たない（ADR-0063）。
pub(crate) fn is_inline_text_element(elements: &HashMap<ElementId, Element>, id: ElementId) -> bool {
    let el = match elements.get(&id) {
        Some(e) => e,
        None => return false,
    };
    if el.kind != ElementKind::Text {
        return false;
    }
    el.parent
        .and_then(|p| elements.get(&p))
        .is_some_and(|p| p.kind == ElementKind::Text)
}

/// IFC ルート = インラインでない `text` 要素。
pub(crate) fn is_ifc_root(elements: &HashMap<ElementId, Element>, id: ElementId) -> bool {
    elements
        .get(&id)
        .is_some_and(|el| el.kind == ElementKind::Text && !is_inline_text_element(elements, id))
}

/// サブツリー内の任意のテキストから、それを含む IFC ルートまで遡る。
pub(crate) fn ifc_root(elements: &HashMap<ElementId, Element>, id: ElementId) -> Option<ElementId> {
    let mut cur = id;
    loop {
        let el = elements.get(&cur)?;
        if el.kind != ElementKind::Text {
            return None;
        }
        if is_ifc_root(elements, cur) {
            return Some(cur);
        }
        cur = el.parent?;
    }
}

#[derive(Clone)]
struct ResolvedTextStyle {
    font_size: f32,
    font_weight: Option<f32>,
    font_family: Option<String>,
    color: Color,
    font_style: Option<FontStyleValue>,
    text_decoration: Option<TextDecorationValue>,
}

impl ResolvedTextStyle {
    fn from_effective(visual: &Visual) -> Self {
        Self {
            font_size: visual.font_size.unwrap_or(16.0),
            font_weight: visual.font_weight,
            font_family: visual.font_family.clone(),
            color: visual.text_color.unwrap_or(Color::BLACK),
            font_style: visual.font_style,
            text_decoration: visual.text_decoration,
        }
    }
}

fn color_to_brush(color: Color) -> TextBrush {
    let c = color.to_array_f32();
    [
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    ]
}

struct CollectCtx<'a> {
    elements: &'a HashMap<ElementId, Element>,
    viewport: (f32, f32),
    text: String,
    spans: Vec<RangedTextSpan>,
    range_map: RangeMap,
}

impl CollectCtx<'_> {
    fn append_segment(&mut self, id: ElementId, slice: &str, style: &ResolvedTextStyle) {
        if slice.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(slice);
        let end = self.text.len();
        self.range_map.insert(start, end, id);
        self.spans.push(RangedTextSpan {
            byte_start: start,
            byte_end: end,
            font_size: style.font_size,
            font_weight: style.font_weight,
            font_family: style.font_family.clone(),
            font_style: style.font_style,
            text_decoration: style.text_decoration,
            brush: color_to_brush(style.color),
        });
    }

    fn walk_ifc_subtree(&mut self, id: ElementId, inherited: InheritedVisualContext) {
        let el = match self.elements.get(&id) {
            Some(e) => e,
            None => return,
        };
        if el.kind != ElementKind::Text {
            return;
        }
        let interaction = InteractionSnapshot::default();
        let effective = effective_visual::resolve_effective(
            &inherited,
            &el.visual,
            &el.viewport_variants,
            self.viewport,
            &el.pseudo_styles,
            &interaction,
            id,
        );
        let style = ResolvedTextStyle::from_effective(&effective);
        if let Some(t) = el.text.as_deref() {
            self.append_segment(id, t, &style);
        }
        let inherited_base = effective_visual::apply_text_inheritance(&inherited, &el.visual);
        let child_inherited = child_inherited_context(
            &inherited,
            el.kind,
            &inherited_base,
            &el.visual,
        );
        let mut children = el.children.clone();
        children.sort_by_key(|cid| {
            self.elements
                .get(cid)
                .map_or(0, |c| c.visual.z_index)
        });
        for child in children {
            self.walk_ifc_subtree(child, child_inherited.clone());
        }
    }
}

/// IFC ルートテキストへ効かせる行クランプ（`max_lines` / `text_overflow`）を解決する。
///
/// CSS の `-webkit-line-clamp` はインラインテキスト自身ではなく、それを内包する
/// **ブロックボックス**に宣言される（例: `titleStyle` は `<button>` に `maxLines:1` を
/// 置き、その子テキストをクランプする）。DOM Mode はカタログの `domExtras` 経由で
/// ボタンへ `-webkit-line-clamp` を載せて成立させるため、Canvas でも同じく
/// **IFC ルート自身に無ければ、それを内包する親ボックスから読む**（issue: todo カード
/// タイトルが Canvas でだけクランプされず折り返す乖離）。`max_lines` がクランプの
/// 唯一のトリガーなので、それを持つ要素の `text_overflow` を一緒に採用する。
fn resolve_line_clamp(
    elements: &HashMap<ElementId, Element>,
    ifc_root_id: ElementId,
) -> (Option<u32>, TextOverflowValue) {
    let Some(el) = elements.get(&ifc_root_id) else {
        return (None, TextOverflowValue::Clip);
    };
    if el.visual.max_lines.is_some() {
        return (el.visual.max_lines, el.visual.text_overflow);
    }
    // IFC ルートを内包するブロックボックス（親）が宣言したクランプを継ぐ。
    if let Some(parent) = el.parent.and_then(|p| elements.get(&p)) {
        if parent.visual.max_lines.is_some() {
            return (parent.visual.max_lines, parent.visual.text_overflow);
        }
    }
    (None, el.visual.text_overflow)
}

/// IFC ルートのサブツリーを単一の Parley レイアウト＋バイト→要素マップに整形する。
pub(crate) fn shape(
    elements: &HashMap<ElementId, Element>,
    ifc_root_id: ElementId,
    max_advance: Option<f32>,
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    viewport: (f32, f32),
) -> TextLayout {
    let mut ctx = CollectCtx {
        elements,
        viewport,
        text: String::new(),
        spans: Vec::new(),
        range_map: RangeMap::default(),
    };
    let root_ctx = InheritedVisualContext {
        ambient: ambient_defaults::ambient_at(elements, ifc_root_id),
        text_local: None,
    };
    ctx.walk_ifc_subtree(ifc_root_id, root_ctx);

    if ctx.text.is_empty() {
        return TextLayout {
            layout: layout_cx
                .ranged_builder(font_cx, "", 1.0, true)
                .build(""),
            runs: Vec::new(),
            font_size: 16.0,
            text: std::sync::Arc::from(""),
            missing_families: Vec::new(),
            range_map: Some(ctx.range_map),
        };
    }

    let (max_lines, text_overflow) = resolve_line_clamp(elements, ifc_root_id);
    let mut layout = build_ranged_text_layout(
        font_cx,
        layout_cx,
        &ctx.text,
        &ctx.spans,
        max_advance,
        max_lines,
        text_overflow,
    );
    layout.range_map = Some(ctx.range_map);
    layout
}

/// Parley の点ヒット → IFC レイアウト内のバイトインデックス。
///
/// Parley の [`Cursor::from_point`] に委譲する。これは
/// [`ClusterSide`](parley::layout::ClusterSide) を尊重し、グリフ後半へのクリックは
/// キャレットをその*後ろ*に置く（RTL や明示的改行のエッジケースも）。素の
/// `Cluster::from_point().text_range().start` はグリフ内のヒットをすべて先頭端へ
/// 吸着させてしまい、キャレットがクリック点に届かない（1クラスタ左にずれる）。
pub(crate) fn byte_index_at_point(layout: &text::TextLayout, local_x: f32, local_y: f32) -> usize {
    use parley::layout::Cursor;
    Cursor::from_point(&layout.layout, local_x, local_y).index()
}

/// `box_hit` が IFC ルートのとき、ボックスレベルのヒット（`box_hit`）を点直下の
/// インラインテキスト要素まで絞り込む。IFC ルートでない・整形済みレイアウトが
/// ない・点が特定のインライン要素に対応しない場合は `box_hit` 自身に戻る。
pub(crate) fn resolve_ifc_inline_hit(
    tree: &crate::element::tree::ElementTree,
    box_hit: ElementId,
    x: f32,
    y: f32,
) -> Option<ElementId> {
    if !is_ifc_root(&tree.elements, box_hit) {
        return Some(box_hit);
    }
    let el = tree.elements.get(&box_hit)?;
    let tl = el.text_layout.as_ref()?;
    let (ex, ey, _, _) = tree.layout.geometry(box_hit)?;
    let byte = byte_index_at_point(tl, x - ex, y - ey);
    if let Some(map) = &tl.range_map {
        if let Some(inline_id) = map.lookup(byte) {
            return Some(inline_id);
        }
    }
    Some(box_hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::tree::Visual;

    fn make_text(id: u64, parent: Option<ElementId>, content: &str, font_size: f32) -> (ElementId, Element) {
        let eid = ElementId::from_u64(id);
        let mut visual = Visual::default();
        visual.font_size = Some(font_size);
        let el = Element {
            kind: ElementKind::Text,
            parent,
            children: Vec::new(),
            layout_style: taffy::Style::default(),
            visual,
            text: Some(content.to_string()),
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
        };
        (eid, el)
    }

    #[test]
    fn shape_concatenates_ifc_subtree_in_document_order() {
        use parley::{FontContext, LayoutContext};
        let mut elements = HashMap::new();
        let (ifc, mut ifc_el) = make_text(1, None, "Hello ", 16.0);
        let (inline, inline_el) = make_text(2, Some(ifc), "world", 20.0);
        ifc_el.children.push(inline);
        elements.insert(ifc, ifc_el);
        elements.insert(inline, inline_el);

        let mut font_cx = FontContext::new();
        let mut layout_cx = LayoutContext::new();
        let layout = shape(
            &elements,
            ifc,
            None,
            &mut font_cx,
            &mut layout_cx,
            (800.0, 600.0),
        );
        assert_eq!(layout.text.as_ref(), "Hello world");
        let map = layout.range_map.as_ref().unwrap();
        assert_eq!(map.lookup(0), Some(ifc));
        assert_eq!(map.lookup(7), Some(inline));
    }
}
