use crate::node::{NodeId, NodeKind, SceneGraph, TextRunData};
use crate::render::RenderImage;

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
    PushTransform {
        transform: [f64; 6],
    },
    PopTransform,
    PushClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
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

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData);

    fn draw_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: &RenderImage,
    );

    fn push_transform(&mut self, transform: [f64; 6]);

    fn pop_transform(&mut self);

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32);

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

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        self.ops.push(DrawOp::DrawTextRun {
            x,
            y,
            color,
            data: data.clone(),
        });
    }

    fn draw_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: &RenderImage,
    ) {
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

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.ops.push(DrawOp::PushClipRect {
            x,
            y,
            width,
            height,
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

    fn draw_text_run(&mut self, _x: f32, _y: f32, _color: [f32; 4], _data: &TextRunData) {}

    fn draw_image(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _data: &RenderImage,
    ) {
    }

    fn push_transform(&mut self, _transform: [f64; 6]) {}

    fn pop_transform(&mut self) {}

    fn push_clip_rect(&mut self, _x: f32, _y: f32, _width: f32, _height: f32) {}

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
        } => {
            let children = node.children.clone();
            painter.push_clip_rect(*x, *y, *width, *height);
            for child_id in children {
                walk_node(graph, child_id, painter);
            }
            painter.pop_clip();
        }
    }
}
