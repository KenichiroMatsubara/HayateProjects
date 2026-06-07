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
