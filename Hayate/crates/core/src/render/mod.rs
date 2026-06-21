mod missing_glyph;
mod painter;
pub mod text_synthesis;

use std::sync::Arc;

use linebender_resource_handle::{Blob, FontData};

pub use missing_glyph::{
    FALLBACK_FONT_CHAIN, MissingGlyphPlaceholder, NOTDEF_GLYPH_ID, is_notdef,
    missing_glyph_placeholder,
};
pub use painter::{
    DrawOp, NullPainter, RecordedFrame, RecordingPainter, ScenePainter, SceneRecorder,
    render_scene_graph,
};

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
