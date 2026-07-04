//! CPU（vello_cpu）レイヤ rasterizer / compositor（tiny-skia crate の
//! `layer_compositor.rs` を写し取った実装）。
//!
//! 座標系は tiny-skia 版と同じ: キャッシュ面は device px（`content_scale` を掛けて raster）。
//! placement の transform は logical scene 座標なので、合成時に `scale(s) ∘ placement ∘ scale(1/s)`
//! へ写して device px の draw 変換にする。
//!
//! 既知の制約: tiny-skia 版は `draw_pixmap`（ラスタ非経由の直接 blit）で合成するのに対し、
//! ここでは vello_cpu に直接 blit API が無いため `RenderContext` に image paint の矩形を
//! 積んで `render_to_pixmap` する形で実装している。機能的には等価だが、composite-only
//! フレームでも vello_cpu の raster パイプラインを都度起動する点は ADR-0125 が理想とする
//! 「合成は安い」からは外れる。性能最適化は本スパイクの範囲外（tiny-skia 側と比較して
//! 問題になるようなら別途対応する）。

use std::collections::HashMap;
use std::sync::Arc;

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer};
use vello_cpu::kurbo::{Affine, Rect};
use vello_cpu::peniko::{Color, Fill};
use vello_cpu::{Image, ImageSource, Pixmap, RenderContext, Resources};

use crate::VelloCpuSceneRenderer;

/// レイヤキャッシュ面は透明クリアで raster する（背景は合成の clear / root レイヤが持つ）。
const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

fn clamp_u16(v: u32) -> u16 {
    v.min(u32::from(u16::MAX)) as u16
}

/// CPU レイヤ rasterizer（`LayerRasterizer` の vello_cpu 実装）。キャッシュ面は device サイズの
/// `Pixmap` を `Arc` で持つ（合成時に `ImageSource::Pixmap` へ渡すのに参照カウントの複製だけで
/// 済ませ、ピクセルバッファの毎フレーム複製を避けるため）。
pub struct VelloCpuLayerRasterizer {
    width: u32,
    height: u32,
    content_scale: f32,
    textures: HashMap<ElementId, Arc<Pixmap>>,
}

impl VelloCpuLayerRasterizer {
    pub fn new(width: u32, height: u32, content_scale: f32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            content_scale,
            textures: HashMap::new(),
        }
    }

    /// サーフェスサイズ / DPR 変更。キャッシュ面は全部作り直しになる（呼び元は planner も invalidate）。
    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.content_scale = content_scale;
        self.textures.clear();
    }
}

impl LayerRasterizer for VelloCpuLayerRasterizer {
    type Texture = Arc<Pixmap>;

    fn rasterize(&mut self, layer: ElementId, scene: &SceneGraph) -> Result<(), String> {
        let (w, h) = (clamp_u16(self.width), clamp_u16(self.height));
        let mut pixmap = Pixmap::new(w, h);
        VelloCpuSceneRenderer::new().render_scene(scene, &mut pixmap, TRANSPARENT, self.content_scale);
        self.textures.insert(layer, Arc::new(pixmap));
        Ok(())
    }

    fn texture(&self, layer: ElementId) -> Option<&Arc<Pixmap>> {
        self.textures.get(&layer)
    }

    fn texture_bytes_per_layer(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
    }

    fn discard(&mut self, layer: ElementId) {
        self.textures.remove(&layer);
    }

    fn discard_all(&mut self) {
        self.textures.clear();
    }
}

/// CPU 合成先（1 フレーム分の Pixmap ＋ clear color）。
pub struct VelloCpuCompositeTarget {
    pub pixmap: Pixmap,
    pub clear: [f32; 4],
}

/// CPU quad compositor（`LayerCompositor` の vello_cpu 実装）。キャッシュ Pixmap を placement
/// （transform / opacity / 軸並行 clip）で image paint として `RenderContext` に積んで合成する。
pub struct VelloCpuLayerCompositor {
    content_scale: f32,
}

impl VelloCpuLayerCompositor {
    pub fn new(content_scale: f32) -> Self {
        Self { content_scale }
    }

    pub fn set_content_scale(&mut self, content_scale: f32) {
        self.content_scale = content_scale;
    }

    /// logical placement 変換 → device px の draw 変換 `scale(s) ∘ placement ∘ scale(1/s)`。
    fn device_transform(&self, t: [f64; 6]) -> Affine {
        let s = f64::from(self.content_scale);
        let [a, b, c, d, e, f] = t;
        // 線形部（a,b,c,d）は scale が相殺、translate（e,f）だけ ×s。
        Affine::new([a, b, c, d, e * s, f * s])
    }
}

impl LayerCompositor for VelloCpuLayerCompositor {
    type Texture = Arc<Pixmap>;
    type Target = VelloCpuCompositeTarget;

    fn composite(
        &mut self,
        target: &mut VelloCpuCompositeTarget,
        quads: &[CompositeQuad<'_, Arc<Pixmap>>],
    ) -> Result<(), String> {
        let (width, height) = (target.pixmap.width(), target.pixmap.height());
        let mut context = RenderContext::new(width, height);
        let mut resources = Resources::new();

        context.set_transform(Affine::IDENTITY);
        context.set_paint(to_color(target.clear));
        context.fill_rect(&Rect::new(0.0, 0.0, f64::from(width), f64::from(height)));

        for quad in quads {
            let has_clip = quad.clip.is_some();
            if let Some([cx, cy, cw, ch]) = quad.clip {
                let s = self.content_scale;
                let mut clip_path = vello_cpu::kurbo::BezPath::new();
                let (x0, y0, x1, y1) = (
                    f64::from(cx * s),
                    f64::from(cy * s),
                    f64::from((cx + cw) * s),
                    f64::from((cy + ch) * s),
                );
                clip_path.move_to((x0, y0));
                clip_path.line_to((x1, y0));
                clip_path.line_to((x1, y1));
                clip_path.line_to((x0, y1));
                clip_path.close_path();
                context.set_transform(Affine::IDENTITY);
                context.set_fill_rule(Fill::NonZero);
                context.push_clip_path(&clip_path);
            }

            let tex_w = quad.texture.width();
            let tex_h = quad.texture.height();
            let brush = Image {
                image: ImageSource::Pixmap(quad.texture.clone()),
                sampler: Default::default(),
            }
            .with_alpha(quad.opacity.clamp(0.0, 1.0));

            context.set_transform(self.device_transform(quad.transform));
            context.set_paint_transform(Affine::IDENTITY);
            context.set_paint(brush);
            context.fill_rect(&Rect::new(0.0, 0.0, f64::from(tex_w), f64::from(tex_h)));
            context.reset_paint_transform();

            if has_clip {
                context.pop_clip_path();
            }
        }

        context.flush();
        context.render_to_pixmap(&mut resources, &mut target.pixmap);
        Ok(())
    }
}

fn to_color(c: [f32; 4]) -> Color {
    let [r, g, b, a] = c;
    Color::new([
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    ])
}
