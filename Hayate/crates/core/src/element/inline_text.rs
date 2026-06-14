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

/// `text` element whose parent is also `text` — no Taffy box (ADR-0063).
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

/// IFC root = `text` element that is not inline.
pub(crate) fn is_ifc_root(elements: &HashMap<ElementId, Element>, id: ElementId) -> bool {
    elements
        .get(&id)
        .is_some_and(|el| el.kind == ElementKind::Text && !is_inline_text_element(elements, id))
}

/// Walk up to the enclosing IFC root for any text in the subtree.
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
        let own = effective_visual::own_with_viewport_variants(
            &el.visual,
            &el.viewport_variants,
            self.viewport,
        );
        let effective = effective_visual::resolve_effective(
            &inherited,
            &own,
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

/// Shape an IFC root subtree into a single Parley layout + byte→element map.
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
            width_constraint: max_advance,
            missing_families: Vec::new(),
            range_map: Some(ctx.range_map),
        };
    }

    let (max_lines, text_overflow) = elements
        .get(&ifc_root_id)
        .map(|el| (el.visual.max_lines, el.visual.text_overflow))
        .unwrap_or((None, TextOverflowValue::Clip));
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

/// Parley point hit → byte index within an IFC layout.
pub(crate) fn byte_index_at_point(layout: &text::TextLayout, local_x: f32, local_y: f32) -> usize {
    use parley::layout::Cluster;
    if let Some((cluster, _)) = Cluster::from_point(&layout.layout, local_x, local_y) {
        cluster.text_range().start
    } else {
        layout.text.len()
    }
}

/// Refine a box-level hit (`box_hit`) into the inline text element under the
/// point, when `box_hit` is an IFC root. Falls back to `box_hit` itself when
/// it's not an IFC root, has no shaped layout, or the point doesn't map to a
/// specific inline element.
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
    let &(ex, ey, _, _) = tree.layout.layout_cache.get(&box_hit)?;
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
