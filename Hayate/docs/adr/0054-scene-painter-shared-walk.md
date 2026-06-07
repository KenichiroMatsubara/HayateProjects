# ScenePainter 共有 walk と scene-renderers crate 分割

**Status: accepted**

**Date: 2026-06-07**

## Context

ADR-0050 は `Scene Renderer` が `SceneGraph` を消費すると決めたが、walk の操作集合と所在を決めていなかった。その結果 `hayate-adapter-web` の Vello / tiny-skia backend が同一の 5 `NodeKind` walk を二重実装している。`RecordingBackend` は SceneGraph を clone するだけで walk を検証できない。

## Decision

### 契約

- **入力は `SceneGraph` のまま**（保持型 retained graph。フラット DisplayList にはしない）。
- **共有 walk は `hayate-core::render` に 1 箇所**：`render_scene_graph(graph, painter)`。
- **backend は `ScenePainter` trait を実装**するだけ。walk は実装しない。

`ScenePainter`（core 内部 seam。host 向け契約ではない）:

```rust
pub trait ScenePainter {
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4], corner_radius: f32);
    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData);
    fn draw_image(&mut self, x: f32, y: f32, w: f32, h: f32, data: &RenderImage);
    fn with_transform(&mut self, transform: [f64; 6], draw: impl FnOnce(&mut Self));
    fn with_clip_rect(&mut self, x: f32, y: f32, w: f32, h: f32, draw: impl FnOnce(&mut Self));
}
```

Group → `with_transform`、Clip → `with_clip_rect`。z-order は `scene_build` 済みの children 順を信頼する。

### 記録と診断

- **`RecordingPainter`**：`flat Vec<DrawOp>` に materialize（`PushTransform` / `PopTransform` / `PushClipRect` / `PopClip` + leaf ops）。
- **`NullPainter`**：no-op。
- **`RecordingBackend` / `NullBackend`（SceneGraph clone 型）は削除**し `RecordingPainter` に統一。

### Crate 分割

```
hayate-core::render           ScenePainter + render_scene_graph + RecordingPainter
crates/scene-renderers/vello  VelloPainter + VelloSceneRenderer::render_scene
crates/scene-renderers/tiny-skia  TinySkiaPainter + TinySkiaSceneRenderer::render_scene
hayate-adapter-web            Render Host（surface/present/resize/selection）+ Platform Adapter
```

- package 名: `hayate-scene-renderer-vello` / `hayate-scene-renderer-tiny-skia`
- **公開 API は `render_scene` のみ**（walk / Painter は crate 内部）。
- **Render Host の web surface（`VelloSurfaceHost` 等）は当面 adapter-web に残留**（H1）。native adapter 追加時に renderer crate へ移管を再検討（decisions-pending Open #2）。

### 命名

- `ScenePainter` = core walk callback（内部）
- `SceneRenderer` = adapter-web の host 向け trait（ADR-0050 の実装単位名と一致）。リネームしない。

## Out of Scope（本 ADR の実装範囲外）

- DisplayList / Raw Layer WIT への `DrawOp` 外部公開
- Render Host surface の renderer crate 移管（Deferred → Open #2）
- `encode` / `present` の分離（同一 SceneGraph 再利用描画）

## Implementation Tasks

### Task A — core: ScenePainter + walk

1. `hayate-core/src/render.rs` に `ScenePainter` trait と `render_scene_graph()` を追加。
2. `DrawOp` enum と `RecordingPainter` / `NullPainter` を実装。
3. `RecordingBackend` / `NullBackend` / `RecordedFrame`（SceneGraph clone 型）を削除。
4. `hayate-core/tests/` に Group / Clip / z-order の pure Rust テスト（`RecordingPainter` 駆動）。

### Task B — scene-renderers crates

1. `crates/scene-renderers/vello/` を新設。`VelloPainter impl ScenePainter`、`VelloSceneRenderer::render_scene`。
2. `crates/scene-renderers/tiny-skia/` を新設。同上。
3. workspace `Cargo.toml` に members 追加。各 crate は GPU 依存を adapter-web から移す。

### Task C — adapter-web 薄型化

1. `backend/vello.rs` / `tiny_skia_backend.rs` から `draw_node` を削除。
2. scene-renderer crate を feature で 1 つだけ link（ADR-0048 二 WASM 維持）。
3. `RenderHost` は surface 初期化・present・resize・selection policy のみ残す。

### Task D — 回帰

1. web-demo（vello / tiny-skia）の描画スモーク。
2. 既存 `render_backends.rs` を `RecordingPainter` ベースに更新。
