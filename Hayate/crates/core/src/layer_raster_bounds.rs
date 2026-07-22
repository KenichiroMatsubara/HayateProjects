use std::collections::HashSet;

use crate::node::{NodeId, NodeKind};
use crate::render::shadow::{SHADOW_REACH_BLUR_FACTOR, SHADOW_REACH_SIGMA_FACTOR};
use crate::render::{transform_verbs, Affine2};
use crate::wire::protocol::{DrawCommand, PathVerb};
use crate::{ElementId, SceneRead};

/// Conservative font-size-relative reach around each shaped glyph origin. This intentionally
/// covers italic/variable-font overhang without making backend glyph-outline policy part of Core.
const TEXT_GLYPH_LEFT_EM: f32 = 0.5;
const TEXT_GLYPH_RIGHT_EM: f32 = 1.5;
const TEXT_GLYPH_ABOVE_EM: f32 = 1.5;
const TEXT_GLYPH_BELOW_EM: f32 = 0.5;

/// Conservative logical-pixel raster extent for one committed compositing layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerRasterBounds {
    pub layer: ElementId,
    pub origin_x: f32,
    pub origin_y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayerRasterBounds {
    pub fn is_finite(self) -> bool {
        [self.origin_x, self.origin_y, self.width, self.height]
            .into_iter()
            .all(f32::is_finite)
    }
}

pub(crate) fn derive_layer_raster_bounds(
    scene: &(impl SceneRead + ?Sized),
    layers: &[ElementId],
) -> Vec<LayerRasterBounds> {
    let layer_set: HashSet<ElementId> = layers.iter().copied().collect();
    layers
        .iter()
        .copied()
        .map(|layer| {
            let mut extent = None;
            if layers.first() == Some(&layer) {
                // The implicit root layer owns every top-level overlay as well as the document
                // root anchor, matching the root Layer Scene projection.
                for &root in scene.roots() {
                    accumulate(
                        scene,
                        root,
                        layer,
                        &layer_set,
                        Affine2::IDENTITY,
                        None,
                        &mut extent,
                    );
                }
            } else if let Some(anchor) = find_anchor(scene, layer) {
                let own_transform = scene.get(anchor).and_then(|anchor| {
                    anchor.children.iter().copied().find(|child| {
                        scene
                            .get(*child)
                            .is_some_and(|node| matches!(node.kind, NodeKind::Group { .. }))
                    })
                });
                if let Some(group) = own_transform.and_then(|id| scene.get(id)) {
                    for &child in &group.children {
                        accumulate(
                            scene,
                            child,
                            layer,
                            &layer_set,
                            Affine2::IDENTITY,
                            None,
                            &mut extent,
                        );
                    }
                } else {
                    accumulate(
                        scene,
                        anchor,
                        layer,
                        &layer_set,
                        Affine2::IDENTITY,
                        None,
                        &mut extent,
                    );
                }
            }
            let (origin_x, origin_y, max_x, max_y) = extent.unwrap_or((0.0, 0.0, 0.0, 0.0));
            LayerRasterBounds {
                layer,
                origin_x,
                origin_y,
                width: (max_x - origin_x).max(0.0),
                height: (max_y - origin_y).max(0.0),
            }
        })
        .collect()
}

fn accumulate(
    scene: &(impl SceneRead + ?Sized),
    id: NodeId,
    target_layer: ElementId,
    layers: &HashSet<ElementId>,
    transform: Affine2,
    clip: Option<Rect>,
    extent: &mut Option<(f32, f32, f32, f32)>,
) {
    let Some(node) = scene.get(id) else { return };
    if let NodeKind::ElementAnchor { element_id } = node.kind {
        if element_id != target_layer && layers.contains(&element_id) {
            return;
        }
    }
    let transform = match &node.kind {
        NodeKind::Group { transform: group } => transform.then(Affine2(group.map(|v| v as f32))),
        _ => transform,
    };
    let clip = match &node.kind {
        NodeKind::Clip {
            x,
            y,
            width,
            height,
            ..
        } => {
            let Some(clip) = intersect(clip, transformed_rect(transform, *x, *y, *width, *height))
            else {
                return;
            };
            Some(clip)
        }
        _ => clip,
    };
    if let NodeKind::Rect {
        x,
        y,
        width,
        height,
        ..
    }
    | NodeKind::RoundedRing {
        x,
        y,
        width,
        height,
        ..
    }
    | NodeKind::DashedBorder {
        x,
        y,
        width,
        height,
        ..
    }
    | NodeKind::InsetBlurredRoundedRect {
        x,
        y,
        width,
        height,
        ..
    }
    | NodeKind::Image {
        x,
        y,
        width,
        height,
        ..
    } = &node.kind
    {
        include_rect(
            extent,
            clipped(transformed_rect(transform, *x, *y, *width, *height), clip),
        );
    }
    if let NodeKind::BlurredRoundedRect {
        x,
        y,
        width,
        height,
        std_dev,
        ..
    } = &node.kind
    {
        let reach =
            (*std_dev * SHADOW_REACH_SIGMA_FACTOR).min(*std_dev * 2.0 * SHADOW_REACH_BLUR_FACTOR);
        include_rect(
            extent,
            clipped(
                transformed_rect(
                    transform,
                    *x - reach,
                    *y - reach,
                    *width + reach * 2.0,
                    *height + reach * 2.0,
                ),
                clip,
            ),
        );
    }
    if let NodeKind::DrawList { x, y, commands } = &node.kind {
        let origin = Affine2::translate(*x, *y);
        let mut ctm = Affine2::IDENTITY;
        let mut stack = Vec::new();
        for command in commands.iter() {
            let geometry = match command {
                DrawCommand::FillPath { verbs, .. } => Some((verbs.as_slice(), 0.0)),
                DrawCommand::StrokePath { verbs, paint } => Some((
                    verbs.as_slice(),
                    paint.stroke_width * ctm.scale_factor() * paint.miter_limit.max(1.0) * 0.5,
                )),
                DrawCommand::Save => {
                    stack.push(ctm);
                    None
                }
                DrawCommand::Restore => {
                    if let Some(saved) = stack.pop() {
                        ctm = saved;
                    }
                    None
                }
                DrawCommand::Translate { dx, dy } => {
                    ctm = ctm.then(Affine2::translate(*dx, *dy));
                    None
                }
                DrawCommand::Rotate { radians } => {
                    ctm = ctm.then(Affine2::rotate(*radians));
                    None
                }
                DrawCommand::Scale { sx, sy } => {
                    ctm = ctm.then(Affine2::scale(*sx, *sy));
                    None
                }
                DrawCommand::Transform { a, b, c, d, e, f } => {
                    ctm = ctm.then(Affine2([*a, *b, *c, *d, *e, *f]));
                    None
                }
                DrawCommand::ClipRect { .. } | DrawCommand::ClipPath { .. } => None,
            };
            let Some((verbs, outset)) = geometry else {
                continue;
            };
            if let Some(rect) = path_rect(&transform_verbs(verbs, origin.then(ctm)), outset) {
                include_rect(extent, clipped(transform_rect(transform, rect), clip));
            }
        }
    }
    if let NodeKind::TextRun { x, y, text_run, .. } = &node.kind {
        let Ok(data) = scene.resources().text_run(*text_run) else {
            return;
        };
        let size = data.font_size.max(0.0);
        for glyph in data.glyphs.iter() {
            include_rect(
                extent,
                clipped(
                    transformed_rect(
                        transform,
                        *x + glyph.x - size * TEXT_GLYPH_LEFT_EM,
                        *y + glyph.y - size * TEXT_GLYPH_ABOVE_EM,
                        size * (TEXT_GLYPH_LEFT_EM + TEXT_GLYPH_RIGHT_EM),
                        size * (TEXT_GLYPH_ABOVE_EM + TEXT_GLYPH_BELOW_EM),
                    ),
                    clip,
                ),
            );
        }
        for decoration in data.decorations.iter() {
            include_rect(
                extent,
                clipped(
                    transformed_rect(
                        transform,
                        *x + decoration.x0,
                        *y + decoration.y - decoration.thickness * 0.5,
                        (decoration.x1 - decoration.x0).max(0.0),
                        decoration.thickness,
                    ),
                    clip,
                ),
            );
        }
    }
    for &child in &node.children {
        accumulate(scene, child, target_layer, layers, transform, clip, extent);
    }
}

fn find_anchor(scene: &(impl SceneRead + ?Sized), layer: ElementId) -> Option<NodeId> {
    fn descend(scene: &(impl SceneRead + ?Sized), id: NodeId, layer: ElementId) -> Option<NodeId> {
        let node = scene.get(id)?;
        if matches!(node.kind, NodeKind::ElementAnchor { element_id } if element_id == layer) {
            return Some(id);
        }
        node.children
            .iter()
            .find_map(|child| descend(scene, *child, layer))
    }
    scene
        .roots()
        .iter()
        .find_map(|root| descend(scene, *root, layer))
}

#[derive(Clone, Copy)]
struct Rect {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

fn transformed_rect(transform: Affine2, x: f32, y: f32, w: f32, h: f32) -> Rect {
    let points = [
        transform.apply(x, y),
        transform.apply(x + w, y),
        transform.apply(x, y + h),
        transform.apply(x + w, y + h),
    ];
    Rect {
        min_x: points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
        min_y: points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
        max_x: points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max),
        max_y: points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max),
    }
}

fn transform_rect(transform: Affine2, rect: Rect) -> Rect {
    transformed_rect(
        transform,
        rect.min_x,
        rect.min_y,
        rect.max_x - rect.min_x,
        rect.max_y - rect.min_y,
    )
}

fn path_rect(verbs: &[PathVerb], outset: f32) -> Option<Rect> {
    let mut points = Vec::new();
    for verb in verbs {
        match *verb {
            PathVerb::MoveTo { x, y } | PathVerb::LineTo { x, y } => points.push((x, y)),
            PathVerb::QuadraticTo { cx, cy, x, y } => points.extend([(cx, cy), (x, y)]),
            PathVerb::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => points.extend([(c1x, c1y), (c2x, c2y), (x, y)]),
            PathVerb::Close => {}
            _ => unreachable!("transform_verbs expands convenience path verbs"),
        }
    }
    if points.is_empty() {
        return None;
    }
    Some(Rect {
        min_x: points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min) - outset,
        min_y: points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min) - outset,
        max_x: points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max) + outset,
        max_y: points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max) + outset,
    })
}

fn intersect(a: Option<Rect>, b: Rect) -> Option<Rect> {
    let Some(a) = a else { return Some(b) };
    let rect = Rect {
        min_x: a.min_x.max(b.min_x),
        min_y: a.min_y.max(b.min_y),
        max_x: a.max_x.min(b.max_x),
        max_y: a.max_y.min(b.max_y),
    };
    (rect.max_x >= rect.min_x && rect.max_y >= rect.min_y).then_some(rect)
}

fn clipped(rect: Rect, clip: Option<Rect>) -> Option<Rect> {
    match clip {
        Some(clip) => intersect(Some(rect), clip),
        None => Some(rect),
    }
}

fn include_rect(extent: &mut Option<(f32, f32, f32, f32)>, rect: Option<Rect>) {
    let Some(rect) = rect else { return };
    if ![rect.min_x, rect.min_y, rect.max_x, rect.max_y]
        .into_iter()
        .all(f32::is_finite)
    {
        return;
    }
    let rect = (rect.min_x, rect.min_y, rect.max_x, rect.max_y);
    *extent = Some(match *extent {
        Some((min_x, min_y, max_x, max_y)) => (
            min_x.min(rect.0),
            min_y.min(rect.1),
            max_x.max(rect.2),
            max_y.max(rect.3),
        ),
        None => rect,
    });
}
