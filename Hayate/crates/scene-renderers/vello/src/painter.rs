use hayate_core::{RenderImage, ScenePainter, TextRunData};
use vello::{
    kurbo::{Affine, Rect, RoundedRect},
    peniko::{Fill, FontData, ImageBrush, color::{AlphaColor, Srgb}},
    Scene,
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

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let font = FontData::new(data.font.data.clone(), data.font.index);
        let glyphs = data.glyphs.iter().map(|glyph| vello::Glyph {
            id: glyph.id,
            x: glyph.x,
            y: glyph.y,
        });
        scene
            .draw_glyphs(&font)
            .font_size(data.font_size)
            .brush(brush)
            .transform(Affine::translate((x as f64, y as f64)))
            .draw(Fill::NonZero, glyphs);
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

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        let scene = self.target();
        let clip = Rect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
        );
        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &clip);
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
