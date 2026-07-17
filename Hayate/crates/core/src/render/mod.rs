mod draw_path;
mod missing_glyph;
mod painter;
pub(crate) mod shadow;
pub(crate) mod text_synthesis;

use linebender_resource_handle::FontData;

pub use linebender_resource_handle::Blob;

pub use draw_path::{
    build_draw_path, transform_verbs, Affine2, DrawFillRule, DrawLineCap, DrawLineJoin, PathSink,
    StrokeStyle,
};
pub use missing_glyph::{
    is_notdef, missing_glyph_placeholder, MissingGlyphPlaceholder, FALLBACK_FONT_CHAIN,
    NOTDEF_GLYPH_ID,
};
pub use painter::{
    render_scene_graph, DrawOp, NullPainter, RecordedFrame, RecordingPainter, ScenePainter,
    SceneRecorder,
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
    /// ピクセルデータ。`RenderFont` と同じく [`Blob`] で保持し、同一画像が生きて
    /// いる間は Blob id が安定する。vello の画像アトラスは Blob id をキーに常駐
    /// 管理するため、id が毎フレーム変わると変化のない画像でも毎フレーム GPU へ
    /// 再アップロードされる（issue #630）。
    pub data: Blob<u8>,
}
