use crate::node::{NodeId, NodeKind, SceneGraph, TextRunData};
use crate::render::draw_path::{transform_verbs, Affine2, DrawFillRule, StrokeStyle};
use crate::render::RenderImage;
use crate::wire::protocol::PathVerb;

#[derive(Debug, Clone)]
pub enum DrawOp {
    FillRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    },
    FillRoundedRing {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    },
    FillBlurredRoundedRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        std_dev: f32,
        color: [f32; 4],
        occluder: Option<crate::node::ShadowOccluder>,
    },
    FillInsetBlurredRoundedRect {
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
    },
    DashedBorder {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    },
    DrawTextRun {
        x: f32,
        y: f32,
        color: [f32; 4],
        data: TextRunData,
    },
    DrawImage {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: RenderImage,
    },
    FillPath {
        x: f32,
        y: f32,
        verbs: Vec<PathVerb>,
        fill_rule: DrawFillRule,
        color: [f32; 4],
    },
    StrokePath {
        x: f32,
        y: f32,
        verbs: Vec<PathVerb>,
        stroke: StrokeStyle,
        color: [f32; 4],
    },
    PushTransform {
        transform: [f64; 6],
    },
    PopTransform,
    PushClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radii: [f32; 4],
    },
    PushClipDrawPath {
        verbs: Vec<PathVerb>,
    },
    PopClip,
}

pub trait ScenePainter {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    );

    fn fill_rounded_ring(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    );

    /// ぼかし角丸矩形（drop shadow）を塗る（issue #657）。`(x, y, width, height)` は影外形
    /// （オフセット・spread 適用済み）、`corner_radius` はその角丸半径、`std_dev` はガウス σ、
    /// `color` は影色（straight RGBA・不透明度適用済み）。
    ///
    /// default 実装は erf シェル近似（[`crate::render::shadow::SHADOW_BLUR_FALLBACK_LAYERS`] 枚）を
    /// `fill_rect` で積む——解析パスを持たないレンダラのピクセル出力は現行のシェル塗りと不変。
    /// 解析パスを持つ painter（vello の `draw_blurred_rounded_rect` / tiny-skia の per-pixel）は
    /// これを override する。
    ///
    /// `occluder`（issue #659）はこの影を覆う不透明 owner の内側矩形。default 実装は最適化を
    /// **無視して影全面を塗る**——覆う不透明ボックスが直後に上から描かれるため最終ピクセルは
    /// どちらでも不変（安全側フォールバック）。解析 painter は覆われる領域を省いて overdraw を削る。
    fn fill_blurred_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        std_dev: f32,
        color: [f32; 4],
        _occluder: Option<crate::node::ShadowOccluder>,
    ) {
        crate::render::shadow::for_each_shadow_shell(
            x,
            y,
            width,
            height,
            corner_radius,
            std_dev,
            color,
            |sx, sy, sw, sh, shell_color, shell_radius| {
                self.fill_rect(sx, sy, sw, sh, shell_color, shell_radius)
            },
        );
    }

    /// inset ぼかしシャドウを塗る（issue #660）。`(x, y, width, height, corner_radius)` は
    /// border-box（塗り領域。角丸クリップは呼び出し側の `Clip` が与える）、`(offset_x, offset_y)`
    /// は影オフセット、`spread` は内側への広がり、`std_dev` はガウス σ、`color` は影色
    /// （straight RGBA・不透明度適用済み）。
    ///
    /// default 実装は同心 `RoundedRing` 帯（erf 減衰の近似）を積む——解析パスを持たないレンダラ
    /// でも現行のシェル塗りと同等に描く。vello（DestOut レイヤ）/ tiny-skia（per-pixel `1 − 被覆`）は
    /// これを override して 1 描画で塗る。
    #[allow(clippy::too_many_arguments)]
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
        crate::render::shadow::for_each_inset_shadow_ring(
            x,
            y,
            width,
            height,
            corner_radius,
            offset_x,
            offset_y,
            spread,
            std_dev,
            color,
            |rx, ry, rw, rh, outer_radius, border_width, ring_color| {
                self.fill_rounded_ring(rx, ry, rw, rh, outer_radius, border_width, ring_color)
            },
        );
    }

    /// ボックス外周に沿って破線ボーダーを描く（`border-style: dashed`）。
    fn stroke_dashed_border(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    );

    /// draw display list のパスを単色で塗る（#724 / ADR-0141）。`(x, y)` は要素の
    /// ボーダーボックス左上（絶対・論理 px）、`verbs` はボーダーボックス相対のパス
    /// 動詞列（fill rule は nonZero）。painter が `(x, y)` の平行移動を適用する。
    /// `fill_rule` は巻き数規則（nonZero / evenOdd・#726）。曲線・便宜形状・arcTo は
    /// [`crate::render::build_draw_path`] が verbs をプリミティブへ展開して painter へ渡す。
    /// 将来の stroke / グラデーション等は verbs・Paint 語彙の拡張として生える。
    fn fill_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        fill_rule: DrawFillRule,
        color: [f32; 4],
    );

    /// draw display list のパスを輪郭描画する（#727）。`stroke` は幅・cap・join・
    /// miterLimit・dash を解決済みの [`StrokeStyle`]。`(x, y)` はボーダーボックス左上、
    /// `verbs` はその相対パス。曲線・便宜形状・arcTo は `fill_path` と同じく
    /// [`crate::render::build_draw_path`] が展開する。
    fn stroke_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        stroke: &StrokeStyle,
        color: [f32; 4],
    );

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData);

    fn draw_image(&mut self, x: f32, y: f32, width: f32, height: f32, data: &RenderImage);

    fn push_transform(&mut self, transform: [f64; 6]);

    fn pop_transform(&mut self);

    /// クリップ領域を push する。`corner_radii`（TL, TR, BR, BL）で角を丸める。
    /// 全て 0 なら矩形クリップ。
    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]);

    /// draw の clipPath / clipRect（#728）。`verbs` は walk が state 変換の元空間へ
    /// 変換済み（ボーダーボックス原点 + draw CTM 適用済み）のパス。既存クリップとの
    /// 交差を 1 つ push し、対応する restore / DrawList 末尾で `pop_clip` される。
    /// 呼び出しごとにちょうど 1 つ push すること（walk のクリップ計数と一致させる）。
    fn push_clip_draw_path(&mut self, verbs: &[PathVerb]);

    fn pop_clip(&mut self);
}

#[derive(Debug, Default)]
pub struct RecordingPainter {
    ops: Vec<DrawOp>,
}

impl RecordingPainter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ops(&self) -> &[DrawOp] {
        &self.ops
    }

    pub fn into_ops(self) -> Vec<DrawOp> {
        self.ops
    }
}

impl ScenePainter for RecordingPainter {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        self.ops.push(DrawOp::FillRect {
            x,
            y,
            width,
            height,
            color,
            corner_radius,
        });
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
        self.ops.push(DrawOp::FillRoundedRing {
            x,
            y,
            width,
            height,
            outer_radius,
            border_width,
            color,
        });
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
        occluder: Option<crate::node::ShadowOccluder>,
    ) {
        // シェルへ展開せず 1 op として記録する（影1個 = ぼかし矩形1 op、issue #657）。
        self.ops.push(DrawOp::FillBlurredRoundedRect {
            x,
            y,
            width,
            height,
            corner_radius,
            std_dev,
            color,
            occluder,
        });
    }

    #[allow(clippy::too_many_arguments)]
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
        // シェルへ展開せず 1 op として記録する（inset 影1個 = 1 op、issue #660）。
        self.ops.push(DrawOp::FillInsetBlurredRoundedRect {
            x,
            y,
            width,
            height,
            corner_radius,
            offset_x,
            offset_y,
            spread,
            std_dev,
            color,
        });
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
        self.ops.push(DrawOp::DashedBorder {
            x,
            y,
            width,
            height,
            outer_radius,
            border_width,
            color,
        });
    }

    fn fill_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        fill_rule: DrawFillRule,
        color: [f32; 4],
    ) {
        self.ops.push(DrawOp::FillPath {
            x,
            y,
            verbs: verbs.to_vec(),
            fill_rule,
            color,
        });
    }

    fn stroke_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        stroke: &StrokeStyle,
        color: [f32; 4],
    ) {
        self.ops.push(DrawOp::StrokePath {
            x,
            y,
            verbs: verbs.to_vec(),
            stroke: stroke.clone(),
            color,
        });
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        self.ops.push(DrawOp::DrawTextRun {
            x,
            y,
            color,
            data: data.clone(),
        });
    }

    fn draw_image(&mut self, x: f32, y: f32, width: f32, height: f32, data: &RenderImage) {
        self.ops.push(DrawOp::DrawImage {
            x,
            y,
            width,
            height,
            data: data.clone(),
        });
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        self.ops.push(DrawOp::PushTransform { transform });
    }

    fn pop_transform(&mut self) {
        self.ops.push(DrawOp::PopTransform);
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        self.ops.push(DrawOp::PushClipRect {
            x,
            y,
            width,
            height,
            corner_radii,
        });
    }

    fn push_clip_draw_path(&mut self, verbs: &[PathVerb]) {
        self.ops.push(DrawOp::PushClipDrawPath {
            verbs: verbs.to_vec(),
        });
    }

    fn pop_clip(&mut self) {
        self.ops.push(DrawOp::PopClip);
    }
}

#[derive(Debug, Default)]
pub struct NullPainter;

impl ScenePainter for NullPainter {
    fn fill_rect(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _color: [f32; 4],
        _corner_radius: f32,
    ) {
    }

    fn fill_rounded_ring(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _outer_radius: f32,
        _border_width: f32,
        _color: [f32; 4],
    ) {
    }

    fn fill_blurred_rounded_rect(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _corner_radius: f32,
        _std_dev: f32,
        _color: [f32; 4],
        _occluder: Option<crate::node::ShadowOccluder>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_inset_blurred_rounded_rect(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _corner_radius: f32,
        _offset_x: f32,
        _offset_y: f32,
        _spread: f32,
        _std_dev: f32,
        _color: [f32; 4],
    ) {
    }

    fn stroke_dashed_border(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _outer_radius: f32,
        _border_width: f32,
        _color: [f32; 4],
    ) {
    }

    fn fill_path(
        &mut self,
        _x: f32,
        _y: f32,
        _verbs: &[PathVerb],
        _fill_rule: DrawFillRule,
        _color: [f32; 4],
    ) {
    }

    fn stroke_path(
        &mut self,
        _x: f32,
        _y: f32,
        _verbs: &[PathVerb],
        _stroke: &StrokeStyle,
        _color: [f32; 4],
    ) {
    }

    fn draw_text_run(&mut self, _x: f32, _y: f32, _color: [f32; 4], _data: &TextRunData) {}

    fn draw_image(&mut self, _x: f32, _y: f32, _width: f32, _height: f32, _data: &RenderImage) {}

    fn push_transform(&mut self, _transform: [f64; 6]) {}

    fn pop_transform(&mut self) {}

    fn push_clip_rect(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _corner_radii: [f32; 4],
    ) {
    }

    fn push_clip_draw_path(&mut self, _verbs: &[PathVerb]) {}

    fn pop_clip(&mut self) {}
}

#[derive(Debug, Clone)]
pub struct RecordedFrame {
    pub clear_color: [f32; 4],
    pub ops: Vec<DrawOp>,
}

#[derive(Debug, Default)]
pub struct SceneRecorder {
    frames: Vec<RecordedFrame>,
}

impl SceneRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, graph: &SceneGraph, clear_color: [f32; 4]) {
        let mut painter = RecordingPainter::new();
        render_scene_graph(graph, &mut painter);
        self.frames.push(RecordedFrame {
            clear_color,
            ops: painter.into_ops(),
        });
    }

    pub fn clear(&mut self, clear_color: [f32; 4]) {
        self.record(&SceneGraph::new(), clear_color);
    }

    pub fn frames(&self) -> &[RecordedFrame] {
        &self.frames
    }
}

pub fn render_scene_graph<P: ScenePainter>(graph: &SceneGraph, painter: &mut P) {
    for &root_id in graph.roots() {
        walk_node(graph, root_id, painter);
    }
}

fn walk_node<P: ScenePainter>(graph: &SceneGraph, id: NodeId, painter: &mut P) {
    let node = match graph.get(id) {
        Some(node) => node,
        None => return,
    };

    match &node.kind {
        NodeKind::Rect {
            x,
            y,
            width,
            height,
            color,
            corner_radius,
        } => painter.fill_rect(*x, *y, *width, *height, *color, *corner_radius),
        NodeKind::RoundedRing {
            x,
            y,
            width,
            height,
            outer_radius,
            border_width,
            color,
        } => painter.fill_rounded_ring(
            *x,
            *y,
            *width,
            *height,
            *outer_radius,
            *border_width,
            *color,
        ),
        NodeKind::BlurredRoundedRect {
            x,
            y,
            width,
            height,
            corner_radius,
            std_dev,
            color,
            occluder,
        } => painter.fill_blurred_rounded_rect(
            *x,
            *y,
            *width,
            *height,
            *corner_radius,
            *std_dev,
            *color,
            *occluder,
        ),
        NodeKind::InsetBlurredRoundedRect {
            x,
            y,
            width,
            height,
            corner_radius,
            offset_x,
            offset_y,
            spread,
            std_dev,
            color,
        } => painter.fill_inset_blurred_rounded_rect(
            *x,
            *y,
            *width,
            *height,
            *corner_radius,
            *offset_x,
            *offset_y,
            *spread,
            *std_dev,
            *color,
        ),
        NodeKind::DashedBorder {
            x,
            y,
            width,
            height,
            outer_radius,
            border_width,
            color,
        } => painter.stroke_dashed_border(
            *x,
            *y,
            *width,
            *height,
            *outer_radius,
            *border_width,
            *color,
        ),
        NodeKind::TextRun { x, y, color, data } => {
            painter.draw_text_run(*x, *y, *color, data.as_ref());
        }
        NodeKind::Image {
            x,
            y,
            width,
            height,
            data,
        } => painter.draw_image(*x, *y, *width, *height, data.as_ref()),
        NodeKind::Group { transform } => {
            let children = node.children.clone();
            painter.push_transform(*transform);
            for child_id in children {
                walk_node(graph, child_id, painter);
            }
            painter.pop_transform();
        }
        NodeKind::Clip {
            x,
            y,
            width,
            height,
            corner_radii,
        } => {
            let children = node.children.clone();
            painter.push_clip_rect(*x, *y, *width, *height, *corner_radii);
            for child_id in children {
                walk_node(graph, child_id, painter);
            }
            painter.pop_clip();
        }
        NodeKind::DrawList { x, y, commands } => {
            use crate::wire::protocol::DrawCommand;
            // canvas の唯一の可変状態: 変換 CTM（ボーダーボックス原点相対）と
            // クリップスタック（#728）。座標操作は verbs へソフト適用し、クリップは
            // painter の既存クリップスタックへ push/pop する。原点 `(x, y)` は
            // fill_path/stroke_path が適用するので fill/stroke は CTM のみ、clip は
            // 原点 + CTM を焼き込む。
            let origin = Affine2::translate(*x, *y);
            let mut ctm = Affine2::IDENTITY;
            let mut save_stack: Vec<(Affine2, usize)> = Vec::new();
            let mut clip_depth: usize = 0;
            for command in commands.iter() {
                match command {
                    DrawCommand::FillPath { verbs, paint } => {
                        let tv = transform_verbs(verbs, ctm);
                        painter.fill_path(
                            *x,
                            *y,
                            &tv,
                            DrawFillRule::from_wire(paint.fill_rule),
                            paint.color,
                        );
                    }
                    DrawCommand::StrokePath { verbs, paint } => {
                        let tv = transform_verbs(verbs, ctm);
                        let mut stroke = StrokeStyle::from_paint(paint);
                        // 幅も CTM のスケールに追従させる（近似・回転/平行移動では不変）。
                        stroke.width *= ctm.scale_factor();
                        painter.stroke_path(*x, *y, &tv, &stroke, paint.color);
                    }
                    DrawCommand::Save => save_stack.push((ctm, clip_depth)),
                    DrawCommand::Restore => {
                        if let Some((t, depth)) = save_stack.pop() {
                            ctm = t;
                            while clip_depth > depth {
                                painter.pop_clip();
                                clip_depth -= 1;
                            }
                        }
                    }
                    DrawCommand::Translate { dx, dy } => {
                        ctm = ctm.then(Affine2::translate(*dx, *dy));
                    }
                    DrawCommand::Rotate { radians } => {
                        ctm = ctm.then(Affine2::rotate(*radians));
                    }
                    DrawCommand::Scale { sx, sy } => {
                        ctm = ctm.then(Affine2::scale(*sx, *sy));
                    }
                    DrawCommand::Transform { a, b, c, d, e, f } => {
                        ctm = ctm.then(Affine2([*a, *b, *c, *d, *e, *f]));
                    }
                    DrawCommand::ClipRect {
                        x: cx,
                        y: cy,
                        width,
                        height,
                    } => {
                        let rect = [
                            PathVerb::MoveTo { x: *cx, y: *cy },
                            PathVerb::LineTo {
                                x: cx + width,
                                y: *cy,
                            },
                            PathVerb::LineTo {
                                x: cx + width,
                                y: cy + height,
                            },
                            PathVerb::LineTo {
                                x: *cx,
                                y: cy + height,
                            },
                            PathVerb::Close,
                        ];
                        let tv = transform_verbs(&rect, origin.then(ctm));
                        painter.push_clip_draw_path(&tv);
                        clip_depth += 1;
                    }
                    DrawCommand::ClipPath { verbs } => {
                        let tv = transform_verbs(verbs, origin.then(ctm));
                        painter.push_clip_draw_path(&tv);
                        clip_depth += 1;
                    }
                }
            }
            // DrawList 内で開いたままのクリップを閉じる（unbalanced save も安全に片付く）。
            while clip_depth > 0 {
                painter.pop_clip();
                clip_depth -= 1;
            }
        }
        NodeKind::ElementAnchor { .. } => {
            let children = node.children.clone();
            for child_id in children {
                walk_node(graph, child_id, painter);
            }
        }
    }
}
