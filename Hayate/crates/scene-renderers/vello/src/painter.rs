use hayate_core::{
    RenderImage, ScenePainter, TextRunData,
    text_synthesis::{embolden_amount_font_units, italic_skew_tangent},
};
use skrifa::raw::{FontRef, TableProvider};
use vello::{
    kurbo::{Affine, Rect, RoundedRect},
    peniko::{
        Fill, FontData, ImageBrush,
        color::{AlphaColor, Srgb},
        kurbo::Diagonal2,
    },
    FontEmbolden, Scene,
};

use crate::to_vello_image;

struct GroupLayer {
    scene: Scene,
    transform: Affine,
}

pub struct VelloPainter<'a> {
    root: &'a mut Scene,
    groups: Vec<GroupLayer>,
    clip_depth: u32,
}

impl<'a> VelloPainter<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        Self {
            root: scene,
            groups: Vec::new(),
            clip_depth: 0,
        }
    }

    fn target(&mut self) -> &mut Scene {
        if let Some(layer) = self.groups.last_mut() {
            &mut layer.scene
        } else {
            self.root
        }
    }
}

impl ScenePainter for VelloPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let x0 = x as f64;
        let y0 = y as f64;
        let x1 = (x + width) as f64;
        let y1 = (y + height) as f64;
        if corner_radius == 0.0 {
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                brush,
                None,
                &Rect::new(x0, y0, x1, y1),
            );
        } else {
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                brush,
                None,
                &RoundedRect::new(x0, y0, x1, y1, corner_radius as f64),
            );
        }
    }

    fn fill_rounded_ring(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    ) {
        let bw = border_width.max(0.0);
        let inner_w = (width - 2.0 * bw).max(0.0);
        let inner_h = (height - 2.0 * bw).max(0.0);
        if inner_w <= 0.0 || inner_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }

        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let outer_r = outer_radius.max(0.0) as f64;
        let inner_r = (outer_radius - bw).max(0.0) as f64;
        let x0 = x as f64;
        let y0 = y as f64;
        let x1 = (x + width) as f64;
        let y1 = (y + height) as f64;
        let ix0 = (x + bw) as f64;
        let iy0 = (y + bw) as f64;
        let ix1 = (x + bw + inner_w) as f64;
        let iy1 = (y + bw + inner_h) as f64;

        use vello::kurbo::{BezPath, RoundedRect, Shape};

        let mut path = BezPath::new();
        path.extend(
            RoundedRect::new(x0, y0, x1, y1, outer_r)
                .path_elements(0.1),
        );
        let mut inner = BezPath::new();
        inner.extend(
            RoundedRect::new(ix0, iy0, ix1, iy1, inner_r)
                .path_elements(0.1),
        );
        inner.reverse_subpaths();
        path.extend(inner);
        scene.fill(Fill::EvenOdd, Affine::IDENTITY, brush, None, &path);
    }

    fn stroke_dashed_border(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    ) {
        let bw = border_width.max(0.0);
        if bw <= 0.0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let half = (bw / 2.0) as f64;
        let inset_w = (width - bw) as f64;
        let inset_h = (height - bw) as f64;
        // Border thicker than the box collapses to a solid fill.
        if inset_w <= 0.0 || inset_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }

        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let inner_r = (outer_radius - bw / 2.0).max(0.0) as f64;
        let x0 = x as f64 + half;
        let y0 = y as f64 + half;
        let x1 = x0 + inset_w;
        let y1 = y0 + inset_h;

        use vello::kurbo::{BezPath, RoundedRect, Shape, Stroke};

        let mut path = BezPath::new();
        path.extend(RoundedRect::new(x0, y0, x1, y1, inner_r).path_elements(0.1));
        let dash = bw as f64 * 2.0;
        let style = Stroke::new(bw as f64).with_dashes(0.0, [dash, dash]);
        scene.stroke(&style, Affine::IDENTITY, brush, None, &path);
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let font = FontData::new(data.font.data.clone(), data.font.index);
        let glyphs = data.glyphs.iter().map(|glyph| vello::Glyph {
            id: glyph.id,
            x: glyph.x,
            y: glyph.y,
        });
        let transform = Affine::translate((x as f64, y as f64));
        let mut builder = scene
            .draw_glyphs(&font)
            .font_size(data.font_size)
            .brush(brush)
            .transform(transform);
        if !data.normalized_coords.is_empty() {
            builder = builder.normalized_coords(data.normalized_coords.as_slice());
        }
        if let Some(degrees) = data.synthesis.skew() {
            let tangent = italic_skew_tangent(degrees) as f64;
            builder = builder.glyph_transform(Some(Affine::new([1.0, 0.0, tangent, 1.0, 0.0, 0.0])));
        }
        if data.synthesis.embolden() {
            let units_per_em = FontRef::from_index(data.font.data.as_ref(), data.font.index)
                .ok()
                .and_then(|font| font.head().ok())
                .map(|head| head.units_per_em())
                .unwrap_or(1000);
            let amount = embolden_amount_font_units(units_per_em);
            builder = builder.font_embolden(FontEmbolden::new(Diagonal2::new(amount, amount)));
        }
        builder.draw(Fill::NonZero, glyphs);

        use vello::kurbo::Shape;
        for deco in &data.decorations {
            let rect = Rect::new(
                deco.x0 as f64,
                (deco.y - deco.thickness * 0.5) as f64,
                deco.x1 as f64,
                (deco.y + deco.thickness * 0.5) as f64,
            );
            scene.fill(Fill::NonZero, transform, brush, None, &rect.to_path(0.1));
        }
    }

    fn draw_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: &RenderImage,
    ) {
        let scene = self.target();
        let img_w = data.width as f32;
        let img_h = data.height as f32;
        let sx = if img_w > 0.0 { width / img_w } else { 1.0 };
        let sy = if img_h > 0.0 { height / img_h } else { 1.0 };
        let transform = Affine::new([sx as f64, 0.0, 0.0, sy as f64, x as f64, y as f64]);
        let brush = ImageBrush::new(to_vello_image(data));
        scene.draw_image(&brush, transform);
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        self.groups.push(GroupLayer {
            scene: Scene::new(),
            transform: Affine::new(transform),
        });
    }

    fn pop_transform(&mut self) {
        let Some(layer) = self.groups.pop() else {
            return;
        };
        if let Some(parent_layer) = self.groups.last_mut() {
            parent_layer.scene.append(&layer.scene, Some(layer.transform));
        } else {
            self.root.append(&layer.scene, Some(layer.transform));
        }
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        let scene = self.target();
        let rect = Rect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
        );
        // Uniform radii (the only shape Hayate currently emits); 0 → rect clip.
        let radius = corner_radii.iter().copied().fold(0.0_f32, f32::max);
        if radius > 0.0 {
            let clip = RoundedRect::from_rect(rect, radius as f64);
            scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &clip);
        } else {
            scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &rect);
        }
        self.clip_depth += 1;
    }

    fn pop_clip(&mut self) {
        if self.clip_depth == 0 {
            return;
        }
        self.target().pop_layer();
        self.clip_depth -= 1;
    }
}
