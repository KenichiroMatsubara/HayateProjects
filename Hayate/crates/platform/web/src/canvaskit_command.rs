use std::collections::HashMap;

use hayate_core::{
    build_draw_path, is_notdef, missing_glyph_placeholder, render_scene_graph, DrawFillRule,
    DrawLineCap, DrawLineJoin, PathSink, PathVerb, RenderImage, RenderImageAlphaType, SceneGraph,
    ScenePainter, ShadowOccluder, StrokeStyle, TextFontSlant, TextRunData,
};

pub(crate) const CLEAR: f32 = 0.0;
pub(crate) const FILL_RECT: f32 = 1.0;
pub(crate) const FILL_ROUNDED_RING: f32 = 2.0;
pub(crate) const DASHED_BORDER: f32 = 3.0;
pub(crate) const FILL_PATH: f32 = 4.0;
pub(crate) const STROKE_PATH: f32 = 5.0;
pub(crate) const DRAW_TEXT: f32 = 6.0;
pub(crate) const DRAW_IMAGE: f32 = 7.0;
pub(crate) const PUSH_TRANSFORM: f32 = 8.0;
pub(crate) const POP_TRANSFORM: f32 = 9.0;
pub(crate) const PUSH_CLIP_RECT: f32 = 10.0;
pub(crate) const PUSH_CLIP_PATH: f32 = 11.0;
pub(crate) const POP_CLIP: f32 = 12.0;
pub(crate) const BLURRED_RECT: f32 = 13.0;
pub(crate) const INSET_BLURRED_RECT: f32 = 14.0;

const PATH_MOVE: f32 = 0.0;
const PATH_LINE: f32 = 1.0;
const PATH_QUAD: f32 = 2.0;
const PATH_CUBIC: f32 = 3.0;
const PATH_CLOSE: f32 = 4.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ResourceKey {
    Font { blob: u64, index: u32 },
    Image { blob: u64 },
}

#[derive(Debug, Default)]
pub(crate) struct ResourceRegistry {
    ids: HashMap<ResourceKey, u32>,
    next_id: u32,
}

impl ResourceRegistry {
    fn resolve(&mut self, key: ResourceKey) -> (u32, bool) {
        if let Some(&id) = self.ids.get(&key) {
            return (id, false);
        }
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("CanvasKit resource id exhausted");
        let id = self.next_id;
        self.ids.insert(key, id);
        (id, true)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ResourcePacket {
    Font {
        id: u32,
        bytes: Vec<u8>,
    },
    Image {
        id: u32,
        width: u32,
        height: u32,
        alpha_type: u32,
        bytes: Vec<u8>,
    },
}

#[derive(Debug, PartialEq)]
pub(crate) struct CanvasKitFrame {
    pub(crate) commands: Vec<f32>,
    pub(crate) resources: Vec<ResourcePacket>,
}

pub(crate) fn encode_scene(
    scene: &SceneGraph,
    clear_color: [f32; 4],
    content_scale: f32,
    registry: &mut ResourceRegistry,
) -> CanvasKitFrame {
    encode_scene_with_origin(scene, clear_color, content_scale, None, registry)
}

pub(crate) fn encode_scene_at(
    scene: &SceneGraph,
    clear_color: [f32; 4],
    content_scale: f32,
    origin_y: f32,
    registry: &mut ResourceRegistry,
) -> CanvasKitFrame {
    encode_scene_with_origin(scene, clear_color, content_scale, Some(origin_y), registry)
}

fn encode_scene_with_origin(
    scene: &SceneGraph,
    clear_color: [f32; 4],
    content_scale: f32,
    origin_y: Option<f32>,
    registry: &mut ResourceRegistry,
) -> CanvasKitFrame {
    let mut frame = CanvasKitFrame {
        commands: vec![
            CLEAR,
            clear_color[0],
            clear_color[1],
            clear_color[2],
            clear_color[3],
        ],
        resources: Vec::new(),
    };
    let mut painter = CommandPainter {
        frame: &mut frame,
        content_scale,
        registry,
    };
    if let Some(origin_y) = origin_y {
        painter.push_transform([1.0, 0.0, 0.0, 1.0, 0.0, -f64::from(origin_y)]);
    }
    render_scene_graph(scene, &mut painter);
    if origin_y.is_some() {
        painter.pop_transform();
    }
    frame
}

pub(crate) fn encode_clear(clear_color: [f32; 4]) -> CanvasKitFrame {
    CanvasKitFrame {
        commands: vec![
            CLEAR,
            clear_color[0],
            clear_color[1],
            clear_color[2],
            clear_color[3],
        ],
        resources: Vec::new(),
    }
}

struct CommandPainter<'a> {
    frame: &'a mut CanvasKitFrame,
    content_scale: f32,
    registry: &'a mut ResourceRegistry,
}

impl CommandPainter<'_> {
    fn scaled(&self, value: f32) -> f32 {
        value * self.content_scale
    }

    fn color(&mut self, color: [f32; 4]) {
        self.frame.commands.extend_from_slice(&color);
    }

    fn path(&mut self, x: f32, y: f32, verbs: &[PathVerb]) {
        let count_at = self.frame.commands.len();
        self.frame.commands.push(0.0);
        let mut sink = EncodedPath {
            commands: &mut self.frame.commands,
            scale: self.content_scale,
            x,
            y,
            count: 0,
        };
        build_draw_path(verbs, &mut sink);
        self.frame.commands[count_at] = sink.count as f32;
    }
}

impl ScenePainter for CommandPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        let scale = self.content_scale;
        self.frame.commands.extend_from_slice(&[
            FILL_RECT,
            x * scale,
            y * scale,
            width * scale,
            height * scale,
            color[0],
            color[1],
            color[2],
            color[3],
            corner_radius * scale,
        ]);
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
        self.frame.commands.extend_from_slice(&[
            FILL_ROUNDED_RING,
            self.scaled(x),
            self.scaled(y),
            self.scaled(width),
            self.scaled(height),
            self.scaled(outer_radius),
            self.scaled(border_width),
        ]);
        self.color(color);
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
        self.frame.commands.extend_from_slice(&[
            BLURRED_RECT,
            self.scaled(x),
            self.scaled(y),
            self.scaled(width),
            self.scaled(height),
            self.scaled(corner_radius),
            self.scaled(std_dev),
        ]);
        self.color(color);
        match occluder {
            Some(value) => self.frame.commands.extend_from_slice(&[
                1.0,
                self.scaled(value.x),
                self.scaled(value.y),
                self.scaled(value.width),
                self.scaled(value.height),
                self.scaled(value.corner_radius),
            ]),
            None => self.frame.commands.push(0.0),
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
        self.frame.commands.extend_from_slice(&[
            INSET_BLURRED_RECT,
            self.scaled(x),
            self.scaled(y),
            self.scaled(width),
            self.scaled(height),
            self.scaled(corner_radius),
            self.scaled(offset_x),
            self.scaled(offset_y),
            self.scaled(spread),
            self.scaled(std_dev),
        ]);
        self.color(color);
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
        self.frame.commands.extend_from_slice(&[
            DASHED_BORDER,
            self.scaled(x),
            self.scaled(y),
            self.scaled(width),
            self.scaled(height),
            self.scaled(outer_radius),
            self.scaled(border_width),
        ]);
        self.color(color);
    }

    fn fill_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        fill_rule: DrawFillRule,
        color: [f32; 4],
    ) {
        self.frame.commands.push(FILL_PATH);
        self.color(color);
        self.frame.commands.push(match fill_rule {
            DrawFillRule::NonZero => 0.0,
            DrawFillRule::EvenOdd => 1.0,
        });
        self.path(x, y, verbs);
    }

    fn stroke_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        stroke: &StrokeStyle,
        color: [f32; 4],
    ) {
        self.frame.commands.push(STROKE_PATH);
        self.color(color);
        self.frame.commands.extend_from_slice(&[
            self.scaled(stroke.width),
            line_cap(stroke.cap),
            line_join(stroke.join),
            stroke.miter_limit,
            stroke.dash.len() as f32,
        ]);
        let scale = self.content_scale;
        self.frame
            .commands
            .extend(stroke.dash.iter().map(|value| value * scale));
        self.frame.commands.push(self.scaled(stroke.dash_offset));
        self.path(x, y, verbs);
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        let key = ResourceKey::Font {
            blob: data.font.data.id(),
            index: data.font.index,
        };
        let (id, is_new) = self.registry.resolve(key);
        if is_new {
            self.frame.resources.push(ResourcePacket::Font {
                id,
                bytes: data.font.data.data().to_vec(),
            });
        }
        self.frame.commands.extend_from_slice(&[
            DRAW_TEXT,
            id as f32,
            self.scaled(x),
            self.scaled(y),
            self.scaled(data.font_size),
        ]);
        self.color(color);
        match data.synthesis.skew_tangent {
            Some(value) => self.frame.commands.extend_from_slice(&[1.0, value]),
            None => self.frame.commands.extend_from_slice(&[0.0, 0.0]),
        }
        match data.synthesis.embolden {
            Some(value) => self.frame.commands.extend_from_slice(&[1.0, value]),
            None => self.frame.commands.extend_from_slice(&[0.0, 0.0]),
        }
        self.frame
            .commands
            .push(data.normalized_coords.len() as f32);
        self.frame
            .commands
            .extend(data.normalized_coords.iter().map(|&coord| coord as f32));
        self.frame.commands.extend_from_slice(&[
            data.font_attributes.weight,
            data.font_attributes.width,
            match data.font_attributes.slant {
                TextFontSlant::Upright => 0.0,
                TextFontSlant::Italic => 1.0,
                TextFontSlant::Oblique => 2.0,
            },
        ]);
        self.frame.commands.push(data.glyphs.len() as f32);
        let scale = self.content_scale;
        for glyph in &data.glyphs {
            self.frame.commands.extend_from_slice(&[
                glyph.id as f32,
                glyph.x * scale,
                glyph.y * scale,
            ]);
        }
        let missing_count = data.glyphs.iter().filter(|glyph| is_notdef(glyph)).count();
        self.frame.commands.push(missing_count as f32);
        for glyph in data.glyphs.iter().filter(|glyph| is_notdef(glyph)) {
            let placeholder = missing_glyph_placeholder(glyph, data.font_size);
            self.frame.commands.extend_from_slice(&[
                placeholder.x * scale,
                placeholder.y * scale,
                placeholder.width * scale,
                placeholder.height * scale,
                placeholder.stroke_width * scale,
            ]);
        }
        self.frame.commands.push(data.decorations.len() as f32);
        for decoration in &data.decorations {
            self.frame.commands.extend_from_slice(&[
                decoration.x0 * scale,
                decoration.x1 * scale,
                decoration.y * scale,
                decoration.thickness * scale,
            ]);
        }
    }

    fn draw_image(&mut self, x: f32, y: f32, width: f32, height: f32, data: &RenderImage) {
        let (id, is_new) = self.registry.resolve(ResourceKey::Image {
            blob: data.data.id(),
        });
        if is_new {
            self.frame.resources.push(ResourcePacket::Image {
                id,
                width: data.width,
                height: data.height,
                alpha_type: match data.alpha_type {
                    RenderImageAlphaType::Opaque => 0,
                    RenderImageAlphaType::Alpha => 1,
                    RenderImageAlphaType::Premultiplied => 2,
                },
                bytes: data.data.data().to_vec(),
            });
        }
        self.frame.commands.extend_from_slice(&[
            DRAW_IMAGE,
            id as f32,
            self.scaled(x),
            self.scaled(y),
            self.scaled(width),
            self.scaled(height),
        ]);
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        let scale = self.content_scale as f64;
        self.frame.commands.extend_from_slice(&[
            PUSH_TRANSFORM,
            transform[0] as f32,
            transform[1] as f32,
            transform[2] as f32,
            transform[3] as f32,
            (transform[4] * scale) as f32,
            (transform[5] * scale) as f32,
        ]);
    }

    fn pop_transform(&mut self) {
        self.frame.commands.push(POP_TRANSFORM);
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        self.frame.commands.extend_from_slice(&[
            PUSH_CLIP_RECT,
            self.scaled(x),
            self.scaled(y),
            self.scaled(width),
            self.scaled(height),
            self.scaled(corner_radii[0]),
            self.scaled(corner_radii[1]),
            self.scaled(corner_radii[2]),
            self.scaled(corner_radii[3]),
        ]);
    }

    fn push_clip_draw_path(&mut self, verbs: &[PathVerb]) {
        self.frame.commands.push(PUSH_CLIP_PATH);
        self.path(0.0, 0.0, verbs);
    }

    fn pop_clip(&mut self) {
        self.frame.commands.push(POP_CLIP);
    }
}

fn line_cap(cap: DrawLineCap) -> f32 {
    match cap {
        DrawLineCap::Butt => 0.0,
        DrawLineCap::Round => 1.0,
        DrawLineCap::Square => 2.0,
    }
}

fn line_join(join: DrawLineJoin) -> f32 {
    match join {
        DrawLineJoin::Miter => 0.0,
        DrawLineJoin::Round => 1.0,
        DrawLineJoin::Bevel => 2.0,
    }
}

struct EncodedPath<'a> {
    commands: &'a mut Vec<f32>,
    scale: f32,
    x: f32,
    y: f32,
    count: usize,
}

impl PathSink for EncodedPath<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.commands.extend_from_slice(&[
            PATH_MOVE,
            (x + self.x) * self.scale,
            (y + self.y) * self.scale,
        ]);
        self.count += 1;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.commands.extend_from_slice(&[
            PATH_LINE,
            (x + self.x) * self.scale,
            (y + self.y) * self.scale,
        ]);
        self.count += 1;
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.commands.extend_from_slice(&[
            PATH_QUAD,
            (cx + self.x) * self.scale,
            (cy + self.y) * self.scale,
            (x + self.x) * self.scale,
            (y + self.y) * self.scale,
        ]);
        self.count += 1;
    }

    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.commands.extend_from_slice(&[
            PATH_CUBIC,
            (c1x + self.x) * self.scale,
            (c1y + self.y) * self.scale,
            (c2x + self.x) * self.scale,
            (c2y + self.y) * self.scale,
            (x + self.x) * self.scale,
            (y + self.y) * self.scale,
        ]);
        self.count += 1;
    }

    fn close(&mut self) {
        self.commands.push(PATH_CLOSE);
        self.count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use hayate_core::{
        Blob, DrawCommand, DrawPaint, Node, NodeKind, RenderFont, RenderGlyph, RenderImage,
        RenderImageAlphaType, RenderImageFormat, SceneGraph, TextDecorationLine,
        TextFontAttributes, TextRunData, TextSynthesis,
    };

    fn text_frame(synthesis: TextSynthesis, normalized_coords: Vec<i16>) -> CanvasKitFrame {
        let data = Arc::new(TextRunData {
            font: RenderFont::new(Blob::from(vec![1, 2, 3, 4]), 0),
            font_size: 12.0,
            font_attributes: TextFontAttributes::default(),
            glyphs: vec![RenderGlyph {
                id: 7,
                x: 1.0,
                y: 2.0,
            }],
            decorations: Vec::new(),
            text: Arc::from("a"),
            synthesis,
            normalized_coords,
        });
        let mut graph = SceneGraph::new();
        graph.insert(Node {
            kind: NodeKind::TextRun {
                x: 3.0,
                y: 4.0,
                color: [1.0; 4],
                data,
            },
            children: Vec::new(),
        });
        encode_scene(&graph, [0.0; 4], 1.0, &mut ResourceRegistry::default())
    }

    #[test]
    fn text_command_preserves_synthesis_and_normalized_variation_coordinates() {
        let regular = text_frame(TextSynthesis::default(), Vec::new());
        let synthesized = text_frame(
            TextSynthesis {
                skew_tangent: Some(0.25),
                embolden: Some(18.0),
            },
            vec![4096, -8192],
        );

        assert_ne!(regular.commands, synthesized.commands);
        assert!(
            synthesized
                .commands
                .windows(10)
                .any(|values| values
                    == [1.0, 0.25, 1.0, 18.0, 2.0, 4096.0, -8192.0, 400.0, 1.0, 0.0,]),
            "text payload must carry synthesis, normalized coordinates, and font attributes: {:?}",
            synthesized.commands,
        );
    }

    #[test]
    fn scroll_band_encoding_translates_content_to_band_local_coordinates() {
        let frame = encode_scene_at(
            &SceneGraph::new(),
            [0.0; 4],
            2.0,
            30.0,
            &mut ResourceRegistry::default(),
        );
        assert_eq!(
            frame.commands,
            vec![
                CLEAR,
                0.0,
                0.0,
                0.0,
                0.0,
                PUSH_TRANSFORM,
                1.0,
                0.0,
                0.0,
                1.0,
                0.0,
                -60.0,
                POP_TRANSFORM
            ],
        );
    }

    #[test]
    fn text_command_preserves_missing_glyphs_and_decorations() {
        let data = Arc::new(TextRunData {
            font: RenderFont::new(Blob::from(vec![1, 2, 3, 4]), 0),
            font_size: 20.0,
            font_attributes: TextFontAttributes::default(),
            glyphs: vec![RenderGlyph {
                id: 0,
                x: 2.0,
                y: 3.0,
            }],
            decorations: vec![TextDecorationLine {
                x0: 1.0,
                x1: 11.0,
                y: 5.0,
                thickness: 2.0,
            }],
            text: Arc::from("missing"),
            synthesis: TextSynthesis::default(),
            normalized_coords: Vec::new(),
        });
        let mut graph = SceneGraph::new();
        graph.insert(Node {
            kind: NodeKind::TextRun {
                x: 7.0,
                y: 9.0,
                color: [1.0; 4],
                data,
            },
            children: Vec::new(),
        });

        let frame = encode_scene(&graph, [0.0; 4], 2.0, &mut ResourceRegistry::default());

        assert!(frame.commands.windows(10).any(|values| values
            .iter()
            .zip([1.0, 0.0, 4.0, 6.0, 1.0, 7.2, -19.6, 18.8, 24.8, 2.4],)
            .all(|(actual, expected)| (actual - expected).abs() < 0.001)));
        assert!(frame
            .commands
            .windows(5)
            .any(|values| values == [1.0, 2.0, 22.0, 10.0, 4.0]));
    }

    #[test]
    fn scene_walk_order_is_preserved_in_the_frame_command_buffer() {
        let mut graph = SceneGraph::new();
        let group = graph.insert(Node {
            kind: NodeKind::Group {
                transform: [1.0, 0.0, 0.0, 1.0, 4.0, 5.0],
            },
            children: Vec::new(),
        });
        graph.insert_child(
            group,
            Node {
                kind: NodeKind::Rect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                    color: [0.1, 0.2, 0.3, 1.0],
                    corner_radius: 5.0,
                },
                children: Vec::new(),
            },
        );
        let mut resources = ResourceRegistry::default();

        let frame = encode_scene(&graph, [0.0, 0.0, 0.0, 1.0], 2.0, &mut resources);

        assert_eq!(
            frame.commands,
            vec![
                CLEAR,
                0.0,
                0.0,
                0.0,
                1.0,
                PUSH_TRANSFORM,
                1.0,
                0.0,
                0.0,
                1.0,
                8.0,
                10.0,
                FILL_RECT,
                2.0,
                4.0,
                6.0,
                8.0,
                0.1,
                0.2,
                0.3,
                1.0,
                10.0,
                POP_TRANSFORM,
            ]
        );
        assert!(frame.resources.is_empty());
    }

    #[test]
    fn clip_path_and_shadow_commands_are_not_dropped_or_reordered() {
        let mut graph = SceneGraph::new();
        let clip = graph.insert(Node {
            kind: NodeKind::Clip {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 40.0,
                corner_radii: [3.0; 4],
            },
            children: Vec::new(),
        });
        graph.insert_child(
            clip,
            Node {
                kind: NodeKind::DrawList {
                    x: 10.0,
                    y: 20.0,
                    commands: Arc::new(vec![DrawCommand::FillPath {
                        verbs: vec![
                            PathVerb::MoveTo { x: 0.0, y: 0.0 },
                            PathVerb::LineTo { x: 2.0, y: 3.0 },
                            PathVerb::Close,
                        ],
                        paint: DrawPaint {
                            color: [1.0, 0.0, 0.0, 1.0],
                            ..DrawPaint::default()
                        },
                    }]),
                },
                children: Vec::new(),
            },
        );
        graph.insert(Node {
            kind: NodeKind::BlurredRoundedRect {
                x: 4.0,
                y: 5.0,
                width: 6.0,
                height: 7.0,
                corner_radius: 2.0,
                std_dev: 1.5,
                color: [0.0, 0.0, 0.0, 0.5],
                occluder: None,
            },
            children: Vec::new(),
        });
        let mut resources = ResourceRegistry::default();

        let frame = encode_scene(&graph, [0.0; 4], 1.0, &mut resources);

        assert_eq!(
            frame.commands,
            vec![
                CLEAR,
                0.0,
                0.0,
                0.0,
                0.0,
                PUSH_CLIP_RECT,
                1.0,
                2.0,
                30.0,
                40.0,
                3.0,
                3.0,
                3.0,
                3.0,
                FILL_PATH,
                1.0,
                0.0,
                0.0,
                1.0,
                0.0,
                3.0,
                PATH_MOVE,
                10.0,
                20.0,
                PATH_LINE,
                12.0,
                23.0,
                PATH_CLOSE,
                POP_CLIP,
                BLURRED_RECT,
                4.0,
                5.0,
                6.0,
                7.0,
                2.0,
                1.5,
                0.0,
                0.0,
                0.0,
                0.5,
                0.0,
            ]
        );
    }

    #[test]
    fn font_and_image_payloads_are_registered_once_and_referenced_each_frame() {
        let font_blob = Blob::from(vec![1, 2, 3, 4]);
        let image_blob = Blob::from(vec![255, 0, 0, 255]);
        let text = Arc::new(TextRunData {
            font: RenderFont::new(font_blob, 0),
            font_size: 12.0,
            font_attributes: TextFontAttributes::default(),
            glyphs: vec![RenderGlyph {
                id: 7,
                x: 1.0,
                y: 2.0,
            }],
            decorations: Vec::new(),
            text: Arc::from("a"),
            synthesis: TextSynthesis::default(),
            normalized_coords: Vec::new(),
        });
        let image = Arc::new(RenderImage {
            width: 1,
            height: 1,
            format: RenderImageFormat::Rgba8,
            alpha_type: RenderImageAlphaType::Alpha,
            data: image_blob,
        });
        let mut graph = SceneGraph::new();
        graph.insert(Node {
            kind: NodeKind::TextRun {
                x: 0.0,
                y: 0.0,
                color: [1.0; 4],
                data: text,
            },
            children: Vec::new(),
        });
        graph.insert(Node {
            kind: NodeKind::Image {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                data: image,
            },
            children: Vec::new(),
        });
        let mut registry = ResourceRegistry::default();

        let first = encode_scene(&graph, [0.0; 4], 1.0, &mut registry);
        let second = encode_scene(&graph, [0.0; 4], 1.0, &mut registry);

        assert!(matches!(
            first.resources[0],
            ResourcePacket::Font { id: 1, .. }
        ));
        assert!(matches!(
            first.resources[1],
            ResourcePacket::Image { id: 2, .. }
        ));
        assert!(
            second.resources.is_empty(),
            "cached resources do not resend payload bytes"
        );
        assert_eq!(
            first.commands, second.commands,
            "each frame keeps stable ResourceId references"
        );
    }

    #[test]
    fn shared_tasks_fixture_encodes_as_a_canvaskit_backend_smoke_frame() {
        let mut tree = hayate_demo_fixtures::tasks_tree("canvaskit");
        let graph = tree.render(0.0);
        let mut registry = ResourceRegistry::default();

        let frame = encode_scene(graph, [1.0; 4], 1.0, &mut registry);

        assert_eq!(frame.commands.first(), Some(&CLEAR));
        assert!(frame.commands.iter().any(|&opcode| opcode == FILL_RECT));
        assert!(frame.commands.iter().any(|&opcode| opcode == DRAW_TEXT));
        assert!(
            frame
                .resources
                .iter()
                .any(|resource| matches!(resource, ResourcePacket::Font { .. })),
            "the shared renderer fixture registers its text resource",
        );
    }
}
