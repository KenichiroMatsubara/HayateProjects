//! リサイズ → `(shape, visual)` dirty の解決を一箇所にまとめる（ADR-0081）。
//!
//! 「リサイズで要素のビューポート条件付きスタイルが変わったか、変わったならどう
//! マークすべきか」をこの 1 つの純粋関数に集約する。呼び出し側は返された集合から
//! dirty を立てるだけでよく、中間の `viewport_dirty` 集合も scan → promote → compare の
//! 暗黙の順序も不要になる。

use std::collections::HashSet;

use crate::element::effective_visual::own_with_viewport_variants;
use crate::element::id::ElementId;
use crate::element::style::{StyleProp, ViewportCondition};
use crate::element::tree::{apply_visual, Visual};

/// 1 要素のリサイズ関連状態。ベース visual とビューポートバリアント。
pub(crate) struct ElementResizeInput<'a> {
    pub id: ElementId,
    pub base: &'a Visual,
    pub variants: &'a [(ViewportCondition, StyleProp)],
}

/// リサイズで変化した要素を、マーク方法ごとに分けたもの。
#[derive(Default, Debug)]
pub(crate) struct ViewportResizeDirty {
    /// バリアントがテキストフィールドに触れる → 再シェイプ（Parley）＋ projection dirty。
    pub shape: HashSet<ElementId>,
    /// バリアントがボックス visual のみ → シーン再 lower（再シェイプ不要）。
    pub visual: HashSet<ElementId>,
}

/// ビューポートリサイズ後にマークが必要な要素を解決する。
///
/// ビューポートバリアントを持つ各要素について、旧/新ビューポートでバリアント解決済みの
/// own-style を比較し（*どの*要素が変化したか）、新ビューポートで有効なバリアントが
/// テキストフィールドに触れるかで shape か visual かを分類する（*どう*マークするか）。
/// バリアントを持たない、または解決結果が不変の要素はスキップする。
pub(crate) fn resolve_resize<'a>(
    elements: impl IntoIterator<Item = ElementResizeInput<'a>>,
    old_viewport: (f32, f32),
    new_viewport: (f32, f32),
) -> ViewportResizeDirty {
    let mut dirty = ViewportResizeDirty::default();
    if old_viewport == new_viewport {
        return dirty;
    }
    for el in elements {
        if el.variants.is_empty() {
            continue;
        }
        if !resolution_changed(el.base, el.variants, old_viewport, new_viewport) {
            continue;
        }
        if variants_touch_text_at(el.variants, new_viewport) {
            dirty.shape.insert(el.id);
        } else {
            dirty.visual.insert(el.id);
        }
    }
    dirty
}

/// ビューポート条件付き own-style が 2 つのビューポート間で異なるか。
fn resolution_changed(
    base: &Visual,
    variants: &[(ViewportCondition, StyleProp)],
    old_viewport: (f32, f32),
    new_viewport: (f32, f32),
) -> bool {
    let old = own_with_viewport_variants(base, variants, old_viewport);
    let new = own_with_viewport_variants(base, variants, new_viewport);
    own_visual_differs(&old, &new)
}

/// `viewport` で有効なバリアントのいずれかがテキストフィールド（font/color など）を
/// 設定するか。設定する場合は paint のみの再 lower ではなく Parley 再シェイプが必要。
fn variants_touch_text_at(
    variants: &[(ViewportCondition, StyleProp)],
    viewport: (f32, f32),
) -> bool {
    let mut probe = Visual::default();
    let mut text_dirty = false;
    for (condition, prop) in variants {
        if condition.matches(viewport.0, viewport.1) {
            apply_visual(&mut probe, prop, &mut text_dirty);
        }
    }
    text_dirty
}

fn own_visual_differs(a: &Visual, b: &Visual) -> bool {
    a.background_color != b.background_color
        || (a.opacity - b.opacity).abs() > f32::EPSILON
        || (a.border_radius - b.border_radius).abs() > f32::EPSILON
        || (a.border_width - b.border_width).abs() > f32::EPSILON
        || a.border_color != b.border_color
        || a.border_style != b.border_style
        || a.box_shadow != b.box_shadow
        || a.overflow != b.overflow
        || a.max_lines != b.max_lines
        || a.text_overflow != b.text_overflow
        || a.text_color != b.text_color
        || a.font_size != b.font_size
        || a.font_weight != b.font_weight
        || a.font_style != b.font_style
        || a.text_decoration != b.text_decoration
        || a.z_index != b.z_index
        || a.font_family != b.font_family
        || a.default_color != b.default_color
        || a.default_font_size != b.default_font_size
        || a.default_font_weight != b.default_font_weight
        || a.default_font_family != b.default_font_family
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn variant(min_width: f32, prop: StyleProp) -> (ViewportCondition, StyleProp) {
        (
            ViewportCondition {
                min_width: Some(min_width),
                ..Default::default()
            },
            prop,
        )
    }

    #[test]
    fn background_variant_crossing_breakpoint_is_a_visual_change() {
        let base = Visual::default();
        let variants = vec![variant(
            768.0,
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        )];
        let id = ElementId::from_u64(1);

        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert!(
            dirty.visual.contains(&id),
            "background variant marks visual"
        );
        assert!(
            !dirty.shape.contains(&id),
            "background variant is not shape"
        );
    }

    #[test]
    fn text_variant_crossing_breakpoint_is_a_shape_change() {
        let base = Visual::default();
        let variants = vec![variant(768.0, StyleProp::FontSize(24.0))];
        let id = ElementId::from_u64(2);

        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert!(dirty.shape.contains(&id), "font-size variant marks shape");
        assert!(
            !dirty.visual.contains(&id),
            "font-size variant is not visual"
        );
    }

    #[test]
    fn element_without_variants_is_skipped() {
        let base = Visual::default();
        let variants: Vec<(ViewportCondition, StyleProp)> = Vec::new();
        let id = ElementId::from_u64(3);

        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert!(dirty.shape.is_empty() && dirty.visual.is_empty());
    }

    #[test]
    fn resize_within_same_breakpoint_changes_nothing() {
        let base = Visual::default();
        let variants = vec![variant(
            768.0,
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        )];
        let id = ElementId::from_u64(4);

        // 両ビューポートとも 768px ブレークポイント以上で、解決結果は不変。
        let dirty = resolve_resize(
            [ElementResizeInput {
                id,
                base: &base,
                variants: &variants,
            }],
            (900.0, 800.0),
            (950.0, 850.0),
        );

        assert!(dirty.shape.is_empty() && dirty.visual.is_empty());
    }

    #[test]
    fn mixed_elements_partition_into_shape_and_visual() {
        let base = Visual::default();
        let bg = vec![variant(
            768.0,
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        )];
        let font = vec![variant(768.0, StyleProp::FontSize(24.0))];
        let none: Vec<(ViewportCondition, StyleProp)> = Vec::new();
        let (visual_id, shape_id, clean_id) = (
            ElementId::from_u64(10),
            ElementId::from_u64(11),
            ElementId::from_u64(12),
        );

        let dirty = resolve_resize(
            [
                ElementResizeInput {
                    id: visual_id,
                    base: &base,
                    variants: &bg,
                },
                ElementResizeInput {
                    id: shape_id,
                    base: &base,
                    variants: &font,
                },
                ElementResizeInput {
                    id: clean_id,
                    base: &base,
                    variants: &none,
                },
            ],
            (500.0, 800.0),
            (900.0, 800.0),
        );

        assert_eq!(dirty.visual, HashSet::from([visual_id]));
        assert_eq!(dirty.shape, HashSet::from([shape_id]));
    }
}
