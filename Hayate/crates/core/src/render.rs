use std::sync::Arc;

use linebender_resource_handle::{Blob, FontData};

use crate::SceneGraph;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderFont {
    pub data: Blob<u8>,
    pub index: u32,
}

impl RenderFont {
    pub fn new(data: Blob<u8>, index: u32) -> Self {
        Self { data, index }
    }

    pub fn to_font_data(&self) -> FontData {
        FontData::new(self.data.clone(), self.index)
    }
}

impl From<FontData> for RenderFont {
    fn from(value: FontData) -> Self {
        Self {
            data: value.data,
            index: value.index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderImageFormat {
    Rgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderImageAlphaType {
    Opaque,
    Alpha,
    Premultiplied,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderImage {
    pub width: u32,
    pub height: u32,
    pub format: RenderImageFormat,
    pub alpha_type: RenderImageAlphaType,
    pub data: Arc<[u8]>,
}

#[derive(Debug, Clone)]
pub struct RecordedFrame {
    pub clear_color: [f32; 4],
    pub scene: SceneGraph,
}

#[derive(Debug, Default)]
pub struct RecordingBackend {
    frames: Vec<RecordedFrame>,
}

impl RecordingBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&mut self, scene: &SceneGraph, clear_color: [f32; 4]) {
        self.frames.push(RecordedFrame {
            clear_color,
            scene: scene.clone(),
        });
    }

    pub fn clear(&mut self, clear_color: [f32; 4]) {
        self.render(&SceneGraph::new(), clear_color);
    }

    pub fn frames(&self) -> &[RecordedFrame] {
        &self.frames
    }
}

#[derive(Debug, Default)]
pub struct NullBackend;

impl NullBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&mut self, _scene: &SceneGraph, _clear_color: [f32; 4]) {}

    pub fn clear(&mut self, _clear_color: [f32; 4]) {}
}
