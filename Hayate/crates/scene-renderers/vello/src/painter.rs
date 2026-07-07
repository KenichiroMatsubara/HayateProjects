use hayate_core::{
    DrawFillRule, PathSink, PathVerb, RenderImage, ScenePainter, ShadowOccluder, TextRunData,
    build_draw_path, is_notdef, missing_glyph_placeholder,
};
use vello::{
    kurbo::{Affine, Rect, RoundedRect},
    peniko::{
        BlendMode, Compose, Fill, FontData, ImageBrush, Mix,
        color::{AlphaColor, Srgb},
        kurbo::Diagonal2,
    },
    FontEmbolden, Scene,
};

use crate::to_vello_image;

/// kurbo `BezPath` を [`PathSink`] として橋渡しする（曲線・便宜形状・arcTo の展開は
/// 共有 [`build_draw_path`] が行う）。
#[derive(Default)]
struct KurboPathSink {
    path: vello::kurbo::BezPath,
}

impl PathSink for KurboPathSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.path
            .quad_to((f64::from(cx), f64::from(cy)), (f64::from(x), f64::from(y)));
    }
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.path.curve_to(
            (f64::from(c1x), f64::from(c1y)),
            (f64::from(c2x), f64::from(c2y)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

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

    fn fill_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        fill_rule: DrawFillRule,
        color: [f32; 4],
    ) {
        let mut sink = KurboPathSink::default();
        build_draw_path(verbs, &mut sink);
        if sink.path.is_empty() {
            return;
        }
        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let rule = match fill_rule {
            DrawFillRule::NonZero => Fill::NonZero,
            DrawFillRule::EvenOdd => Fill::EvenOdd,
        };
        // verbs はボーダーボックス相対。原点 `(x, y)` は平行移動で与える。
        scene.fill(
            rule,
            Affine::translate((f64::from(x), f64::from(y))),
            brush,
            None,
            &sink.path,
        );
    }

    fn fill_blurred_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        std_dev: f32,
        color: [f32; 4],
        occluder: Option<ShadowOccluder>,
    ) {
        // vendor 済みの解析ガウス経路（Evan-Wallace 式）へ直結する。影1個 = 1描画で、
        // シェル近似の 11 パス emit を置き換える（issue #657）。`rect` は鋭い角丸矩形
        // （影外形）、ぼかしはこの周りに解析的に広がる。
        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let rect = Rect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
        );
        match occluder {
            // 不透明 owner が覆う border-box 内側を塗りから除外する（issue #659）。ぼかしを
            // 「外側 bbox − occluder 角丸矩形」のリング形状にクリップして描く。覆われて見えない
            // 中央を GPU がラスタしない（出力ピクセルは不変）。
            Some(occ) if occ.width > 0.0 && occ.height > 0.0 => {
                use vello::kurbo::{BezPath, RoundedRect, Shape};
                // ぼかしの可視範囲を覆う外側形状（vendor の打ち切り 2.5σ に一致）。
                let kernel = 2.5 * std_dev as f64;
                let outer = rect.inflate(kernel, kernel);
                let mut path = BezPath::new();
                path.extend(outer.path_elements(0.1));
                let mut hole = BezPath::new();
                hole.extend(
                    RoundedRect::new(
                        occ.x as f64,
                        occ.y as f64,
                        (occ.x + occ.width) as f64,
                        (occ.y + occ.height) as f64,
                        occ.corner_radius as f64,
                    )
                    .path_elements(0.1),
                );
                // 逆巻きの穴を足し、NonZero 塗りでリング（外側マイナス内側）にする。
                hole.reverse_subpaths();
                path.extend(hole);
                scene.draw_blurred_rounded_rect_in(
                    &path,
                    Affine::IDENTITY,
                    rect,
                    brush,
                    corner_radius as f64,
                    std_dev as f64,
                );
            }
            _ => {
                scene.draw_blurred_rounded_rect(
                    Affine::IDENTITY,
                    rect,
                    brush,
                    corner_radius as f64,
                    std_dev as f64,
                );
            }
        }
    }

    fn fill_inset_blurred_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        offset_x: f32,
        offset_y: f32,
        spread: f32,
        std_dev: f32,
        color: [f32; 4],
    ) {
        // inset 影 = border-box を影色でベタ塗り → 内側 hole を解析ぼかしで DestOut して削る
        // （issue #660）。結果は `color.a · (1 − hole_coverage)`。border-box への角丸クリップは
        // 呼び出し側 `Clip` が与える。影1個 = 1描画（+ 塗り）。
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let scene = self.target();
        let brush = AlphaColor::<Srgb>::new(color);
        let border = RoundedRect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
            corner_radius.max(0.0) as f64,
        );
        // 1. border-box を影色で塗る。
        scene.fill(Fill::NonZero, Affine::IDENTITY, brush, None, &border);

        // 2. hole をぼかして DestOut → 塗りを `1 − hole_coverage` に削る。
        let spread = spread.max(0.0);
        let hole_w = width - 2.0 * spread;
        let hole_h = height - 2.0 * spread;
        if hole_w > 0.0 && hole_h > 0.0 {
            let hx = (x + offset_x + spread) as f64;
            let hy = (y + offset_y + spread) as f64;
            let hole = Rect::new(hx, hy, hx + hole_w as f64, hy + hole_h as f64);
            let hole_radius = (corner_radius - spread).max(0.0) as f64;
            let blend = BlendMode::new(Mix::Normal, Compose::DestOut);
            scene.push_layer(Fill::NonZero, blend, 1.0, Affine::IDENTITY, &border);
            // DestOut は src のアルファ（= hole 被覆）だけを見るので、不透明ブラシで塗る。
            let opaque = AlphaColor::<Srgb>::new([0.0, 0.0, 0.0, 1.0]);
            scene.draw_blurred_rounded_rect(Affine::IDENTITY, hole, opaque, hole_radius, std_dev as f64);
            scene.pop_layer();
        }
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
        // ボーダーが箱より太い場合は塗りつぶしに退化する。
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
        // 実グリフ描画では `.notdef` をスキップする。フォントの無言の箱ではなく、
        // 意図的なプレースホルダ箱として後で描くため。
        let glyphs = data
            .glyphs
            .iter()
            .filter(|glyph| !is_notdef(glyph))
            .map(|glyph| vello::Glyph {
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
        if let Some(tangent) = data.synthesis.skew_tangent {
            let tangent = tangent as f64;
            builder = builder.glyph_transform(Some(Affine::new([1.0, 0.0, tangent, 1.0, 0.0, 0.0])));
        }
        if let Some(amount) = data.synthesis.embolden {
            let amount = amount as f64;
            builder = builder.font_embolden(FontEmbolden::new(Diagonal2::new(amount, amount)));
        }
        builder.draw(Fill::NonZero, glyphs);

        use vello::kurbo::{Shape, Stroke};
        // フォントが供給できないコードポイント用の意図的なプレースホルダ箱。
        // `missing_glyph_placeholder` 経由で tiny-skia バックエンドと一致させる。
        for glyph in data.glyphs.iter().filter(|glyph| is_notdef(glyph)) {
            let ph = missing_glyph_placeholder(glyph, data.font_size);
            if ph.width <= 0.0 || ph.height <= 0.0 {
                continue;
            }
            let rect = Rect::new(
                ph.x as f64,
                ph.y as f64,
                (ph.x + ph.width) as f64,
                (ph.y + ph.height) as f64,
            );
            let style = Stroke::new(ph.stroke_width as f64);
            scene.stroke(&style, transform, brush, None, &rect.to_path(0.1));
        }
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
        // 均一な角丸半径（Hayate が現在発行する唯一の形状）。0 なら矩形クリップ。
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

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{render_scene_graph, Node, NodeKind, SceneGraph};

    /// ぼかし角丸矩形プリミティブ 1 個が、解析ガウス経路で **1 描画** にエンコードされること
    /// （issue #657：シェル近似の 11 パス → 1 描画）。GPU デバイス無しで Scene の encoding を
    /// 直接検査する。フォールバックのシェル近似なら 11 枚の COLOR fill になる。
    #[test]
    fn blurred_rounded_rect_encodes_a_single_draw() {
        let mut sg = SceneGraph::new();
        sg.insert(Node {
            kind: NodeKind::BlurredRoundedRect {
                x: 8.0,
                y: 8.0,
                width: 50.0,
                height: 50.0,
                corner_radius: 8.0,
                std_dev: 3.0,
                color: [0.0, 0.0, 0.0, 0.5],
                occluder: None,
            },
            children: Vec::new(),
        });

        let mut scene = Scene::new();
        let mut painter = VelloPainter::new(&mut scene);
        render_scene_graph(&sg, &mut painter);

        assert_eq!(
            scene.encoding().draw_tags.len(),
            1,
            "the analytic path must encode one draw per shadow, not a stack of shell fills"
        );
    }

    /// occluder 付き（不透明 owner）でも解析ドロー1個へエンコードされる（issue #659）。ぼかしは
    /// リング形状にクリップされるだけで、描画コマンド数は増えない。
    #[test]
    fn occluded_blurred_rounded_rect_still_encodes_a_single_draw() {
        use hayate_core::ShadowOccluder;
        let mut sg = SceneGraph::new();
        sg.insert(Node {
            kind: NodeKind::BlurredRoundedRect {
                x: 8.0,
                y: 8.0,
                width: 50.0,
                height: 50.0,
                corner_radius: 8.0,
                std_dev: 3.0,
                color: [0.0, 0.0, 0.0, 0.5],
                occluder: Some(ShadowOccluder {
                    x: 8.0,
                    y: 8.0,
                    width: 50.0,
                    height: 50.0,
                    corner_radius: 8.0,
                }),
            },
            children: Vec::new(),
        });

        let mut scene = Scene::new();
        let mut painter = VelloPainter::new(&mut scene);
        render_scene_graph(&sg, &mut painter);

        assert_eq!(
            scene.encoding().draw_tags.len(),
            1,
            "clipping the shadow to a ring must not add extra draws"
        );
    }

    /// inset 影が解析ぼかし経路で描かれる（シェルリングではない・issue #660）。encoding に
    /// ぼかし矩形ドロー（`DrawTag::BLUR_RECT` = 0x2d4）が 1 個現れることを確認する。
    #[test]
    fn inset_blurred_rounded_rect_uses_the_analytic_blur_draw() {
        const BLUR_RECT_TAG: u32 = 0x2d4;
        let mut sg = SceneGraph::new();
        sg.insert(Node {
            kind: NodeKind::InsetBlurredRoundedRect {
                x: 10.0,
                y: 10.0,
                width: 60.0,
                height: 60.0,
                corner_radius: 12.0,
                offset_x: 0.0,
                offset_y: 0.0,
                spread: 2.0,
                std_dev: 4.0,
                color: [0.0, 0.0, 0.0, 0.6],
            },
            children: Vec::new(),
        });

        let mut scene = Scene::new();
        let mut painter = VelloPainter::new(&mut scene);
        render_scene_graph(&sg, &mut painter);

        let blur_draws = scene
            .encoding()
            .draw_tags
            .iter()
            .filter(|t| t.0 == BLUR_RECT_TAG)
            .count();
        assert_eq!(
            blur_draws, 1,
            "the inset shadow must be drawn with one analytic gaussian blur, not shell rings"
        );
    }
}
