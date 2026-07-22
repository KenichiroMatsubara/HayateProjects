// wire モジュールに include する生成物（protocol.rs / dispatch.rs）は絶対パス
// `hayate_core::` で core 型を参照する（アダプタと同一ソースを共有するため）。
// 自クレートをその名前で参照できるよう self を別名にする。
extern crate self as hayate_core;

pub mod audio_output;
pub mod battery;
pub mod biometric;
pub mod capability;
pub mod color;
mod committed_frame;
pub mod connectivity;
pub mod device_info;
pub mod element;
pub mod file_picker;
pub mod geolocation;
pub mod haptics;
pub mod key_value_store;
mod layer_raster_bounds;
mod layer_scene;
pub mod local_notification;
pub mod node;
pub mod qr_scanner;
pub mod render;
pub mod render_scale;
#[cfg(any(debug_assertions, feature = "scene-validation"))]
mod scene_validation;
pub mod scroll;
pub mod secure_storage;
pub mod sensors;
pub mod share;
pub mod subscription;
pub mod surface;
pub mod surface_lifecycle;
mod text_resources;
pub mod touch_input;
pub mod url_launcher;
pub mod viewport_metrics;
pub mod wire;

pub use audio_output::{
    AudioFormat, AudioOutput, DEFAULT_BUFFER_FRAMES, DEFAULT_CHANNEL_COUNT, DEFAULT_SAMPLE_RATE_HZ,
};
// capability scaffold（ADR-0119）。契約の正本は Core。leaf stub は `Unimplemented` を返す。
// wave-2 ストリーム capability（ADR-0120）: battery が共有契約土台のトレーサーバレット。
pub use battery::{Battery, BatteryStatus};
// draw display list（#724 / ADR-0141/0142）: wire 生成物の decode 型を crate 根で公開する
// （生成 sink / アダプタ / painter が `hayate_core::DrawCommand` で参照する）。
pub use biometric::Biometric;
pub use capability::CapabilityError;
pub use wire::protocol::{DrawCommand, DrawPaint, PathVerb};
// wave-2 ストリーム capability（ADR-0120）。connectivity は battery の共有契約土台を再利用する。
pub use connectivity::{Connectivity, ConnectivityProvider};
// clipboard は capability に含めない: 編集境界 `element::clipboard::Clipboard`（ADR-0097 /
// ADR-0014 の Platform Adapter 責務）が所有済み。同一 OS API への 2 重抽象を避ける（ADR-0119）。
pub use device_info::{DeviceInfo, DeviceInfoProvider};
pub use file_picker::{FileFilter, FilePicker, PickedFile, SavePath};
// wave-2 ストリーム capability（ADR-0120）。geolocation は battery の共有契約土台を再利用する
// （権限は据え置き — `PermissionDenied` は足さない・ADR-0119/0120）。
pub use geolocation::{Geolocation, Position};
pub use haptics::{HapticKind, Haptics};
pub use key_value_store::KeyValueStore;
pub use local_notification::{LocalNotification, LocalNotifications};
// async-UI 一発取得 capability（ADR-0125）。file_picker と同型。Mobile Family Adapter の
// `MobileQrScanner` が iOS/Android leaf を単一 API に解決する（web は family-of-1 で別 leaf）。
pub use qr_scanner::{QrScanner, ScannedCode};
pub use secure_storage::SecureStorage;
// wave-2 ストリーム capability（ADR-0120）。sensors は battery の共有契約土台を再利用しつつ、単一
// trait ＋ `SensorKind` 引数という一段違う形で全 sensor を出し分ける（高頻度 drain・全件保持）。
pub use sensors::{SensorKind, SensorSample, Sensors};
pub use share::Share;
// wave-2 ストリーム capability 契約土台（ADR-0120）。Core 所有の RAII 購読ハンドルと producer 側。
pub use color::Color;
pub use committed_frame::{CommittedFrame, ScrollCompositorInput};
pub use element::chrome_tuning::ChromeTuning;
pub use element::{
    apply_command, apply_ime_action, event_document_kind, AccessibilityPoll, AlignContentValue,
    AlignSelfValue, AlignValue, BorderStyleValue, BoxSizingValue, CharacterBounds, Clipboard,
    CompositionClause, CompositionUnderline, CursorValue, Dimension, DimensionUnit, Direction,
    DisplayValue, DocumentEventKind, EditIntent, EditState, ElementId, ElementKind, ElementTree,
    Event, EventDelivery, FlexDirectionValue, FlexWrapValue, FontFetcher, FontStyleValue,
    Granularity, GridAutoFlowValue, GridLineValue, GridPlacementValue, ImeAction, ImeBridge,
    ImeBuffer, ImeCommand, ImePresentation, InputModality, InteractionIntent, InteractionResult,
    JustifyItemsValue, JustifySelfValue, JustifyValue, LayoutPass, ListenerId, OverflowMenu,
    OverflowValue, PointerKind, PointerMoveResult, PointerRouting, PositionValue, Preedit,
    PseudoState, ResolvedElement, Selection, SelectionChromeStyle, SelectionHandle,
    SelectionHandleEnd, SelectionHandles, SelectionPoint, SelectionToolbar, Shadow, StyleProp,
    StylePropKind, TextDecorationValue, TextOverflowValue, ToolbarAction, ToolbarButton,
    ToolbarHit, ToolbarRect, TransitionTimingValue, UserSelectValue, ViewportCondition,
};
pub use layer_raster_bounds::LayerRasterBounds;
pub use layer_scene::{
    compose, LayerPlacement, LayerScene, LayerSceneKind, LayerTopology, IDENTITY,
};
pub use node::{
    Node, NodeId, NodeKind, SceneChangeJournal, SceneCommitStats, SceneGraph, SceneRead,
    SceneSnapshot, ShadowOccluder, TextDecorationLine, TextFontAttributes, TextFontSlant,
    TextRunData, TextSynthesis,
};
pub use render::{
    build_draw_path, is_notdef, missing_glyph_placeholder, render_scene_graph, transform_verbs,
    Affine2, Blob, DrawFillRule, DrawLineCap, DrawLineJoin, DrawOp, MissingGlyphPlaceholder,
    NullPainter, PathSink, RecordedFrame, RecordingPainter, RenderFont, RenderGlyph, RenderImage,
    RenderImageAlphaType, RenderImageFormat, ScenePainter, SceneRecorder, StrokeStyle,
    FALLBACK_FONT_CHAIN, NOTDEF_GLYPH_ID,
};
pub use render_scale::{
    effective_content_scale, hit_test_logical, RenderScaleDriver, RenderScaleGovernor,
};
#[cfg(any(debug_assertions, feature = "scene-validation"))]
pub use scene_validation::{
    validate_scene_graph, SceneGraphValidator, SceneValidationError, SceneValidationReport,
};
pub use scroll::{MoveOutcome, ScrollGesture, ScrollPhysicsProfile, ScrollPhysicsTuning};
pub use subscription::{Subscription, SubscriptionSource};
pub use surface::Surface;
pub use surface_lifecycle::{SurfaceLifecycleAction, SurfaceLifecycleEvent, SurfaceLifecycleState};
pub use text_resources::{
    FontInstance, FontInstanceId, ResourceLookupError, ResourceSweepStats, SceneResources,
    TextResourcePolicy, TextRun, TextRunId, DEFAULT_TEXT_RESOURCE_SWEEP_THRESHOLD,
};
pub use touch_input::{translate_touch, PointerInput, TouchAction};
pub use url_launcher::UrlLauncher;
pub use viewport_metrics::{viewport_size_changed, ViewportMetrics};
