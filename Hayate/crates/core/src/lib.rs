// wire モジュールに include する生成物（protocol.rs / dispatch.rs）は絶対パス
// `hayate_core::` で core 型を参照する（アダプタと同一ソースを共有するため）。
// 自クレートをその名前で参照できるよう self を別名にする。
extern crate self as hayate_core;

pub mod audio_output;
pub mod biometric;
pub mod capability;
pub mod color;
pub mod device_info;
pub mod element;
pub mod file_picker;
pub mod haptics;
pub mod key_value_store;
pub mod local_notification;
pub mod node;
pub mod render;
pub mod scroll;
pub mod secure_storage;
pub mod share;
pub mod surface_lifecycle;
pub mod touch_input;
pub mod url_launcher;
pub mod viewport_metrics;
pub mod wire;

pub use audio_output::{
    AudioFormat, AudioOutput, DEFAULT_BUFFER_FRAMES, DEFAULT_CHANNEL_COUNT, DEFAULT_SAMPLE_RATE_HZ,
};
// capability scaffold（ADR-0119）。契約の正本は Core。leaf stub は `Unimplemented` を返す。
pub use biometric::Biometric;
pub use capability::CapabilityError;
// clipboard は capability に含めない: 編集境界 `element::clipboard::Clipboard`（ADR-0097 /
// ADR-0014 の Platform Adapter 責務）が所有済み。同一 OS API への 2 重抽象を避ける（ADR-0119）。
pub use device_info::{DeviceInfo, DeviceInfoProvider};
pub use file_picker::{FileFilter, FilePicker, PickedFile, SavePath};
pub use haptics::{HapticKind, Haptics};
pub use key_value_store::KeyValueStore;
pub use local_notification::{LocalNotification, LocalNotifications};
pub use secure_storage::SecureStorage;
pub use share::Share;
pub use url_launcher::UrlLauncher;
pub use color::Color;
pub use element::chrome_tuning::ChromeTuning;
pub use element::{
    AlignContentValue, AlignSelfValue, AlignValue, BorderStyleValue, BoxSizingValue, CursorValue, Dimension,
    DimensionUnit,
    DisplayValue,
    DocumentEventKind,
    ElementId, ElementKind,
    ElementTree, Event, EventDelivery, FlexDirectionValue, FlexWrapValue, GridAutoFlowValue, GridLineValue, GridPlacementValue, JustifyItemsValue, JustifySelfValue, JustifyValue, LayoutPass,
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
pub use node::{Node, NodeId, NodeKind, SceneGraph, TextDecorationLine, TextRunData, TextSynthesis};
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
    render_scene_graph,
};
