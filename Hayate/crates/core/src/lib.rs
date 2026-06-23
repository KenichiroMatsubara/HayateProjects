// wire モジュールに include する生成物（protocol.rs / dispatch.rs）は絶対パス
// `hayate_core::` で core 型を参照する（アダプタと同一ソースを共有するため）。
// 自クレートをその名前で参照できるよう self を別名にする。
extern crate self as hayate_core;

pub mod color;
pub mod element;
pub mod node;
pub mod render;
pub mod scroll;
pub mod surface_lifecycle;
pub mod touch_input;
pub mod viewport_metrics;
pub mod wire;

pub use color::Color;
pub use element::chrome_tuning::ChromeTuning;
pub use element::{
    AlignContentValue, AlignSelfValue, AlignValue, BorderStyleValue, CursorValue, Dimension,
    DimensionUnit,
    DisplayValue,
    DocumentEventKind,
    ElementId, ElementKind,
    ElementTree, Event, EventDelivery, FlexDirectionValue, FlexWrapValue, JustifyValue, LayoutPass,
    ListenerId, OverflowValue, PositionValue,
    event_document_kind,
    CharacterBounds, Clipboard, CompositionClause, CompositionUnderline, Direction, EditIntent,
    EditState, FontStyleValue, Granularity,
    ImeBridge, ImePresentation, InputModality, PointerKind, PointerMoveResult, Preedit, PseudoState,
    ResolvedElement,
    Selection, SelectionChromeStyle, SelectionHandle, SelectionHandleEnd, SelectionHandles,
    SelectionPoint, SelectionToolbar, ToolbarAction, ToolbarButton, ToolbarRect,
    Shadow,
    StyleProp, StylePropKind, TextDecorationValue, TextOverflowValue, TransitionTimingValue,
    UserSelectValue, ViewportCondition,
};
pub use node::{Node, NodeId, NodeKind, SceneGraph, TextDecorationLine, TextRunData};
pub use scroll::{
    MoveOutcome, ScrollGesture, ScrollPhysicsProfile, ScrollPhysicsTuning,
};
pub use surface_lifecycle::{
    SurfaceLifecycleAction, SurfaceLifecycleEvent, SurfaceLifecycleState,
};
pub use touch_input::{translate_touch, PointerInput, TouchAction};
pub use viewport_metrics::{viewport_size_changed, ViewportMetrics};
pub use render::{
    DrawOp, FALLBACK_FONT_CHAIN, MissingGlyphPlaceholder, NOTDEF_GLYPH_ID, NullPainter,
    RecordedFrame, RecordingPainter, RenderFont, RenderGlyph, RenderImage, RenderImageAlphaType,
    RenderImageFormat, ScenePainter, SceneRecorder, is_notdef, missing_glyph_placeholder,
    render_scene_graph, text_synthesis,
};
