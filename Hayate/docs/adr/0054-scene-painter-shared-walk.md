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
    fn push_transform(&mut self, transform: [f64; 6]);
    fn pop_transform(&mut self);
    fn push_clip_rect(&mut self, x: f32, y: f32, w: f32, h: f32);
    fn pop_clip(&mut self);
}
```

Group → `push_transform` / `pop_transform`、Clip → `push_clip_rect` / `pop_clip`。walk は対で呼ぶ。z-order は `scene_build` 済みの children 順を信頼する。grill 時の RAII scope（`with_*`）は Vello sub-scene の lifetime と両立しないため、trait は明示 stack API を採用した。

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
- **公開 API は `render_scene` と、Render Host が surface/present に使う補助のみ**（walk / `ScenePainter` 実装は crate 内部）。
  - 公開可: `*SceneRenderer::render_scene`、surface/present 補助（vello: `VelloRenderTarget` / `create_target_view` / `create_blitter`、tiny-skia: `premultiplied_to_straight`）。adapter-web の Render Host（本 ADR が adapter-web に置くと決めた層）が消費する。
  - 非公開: `VelloPainter` / `TinySkiaPainter`（`ScenePainter` 実装＝core walk のコールバック）と内部 premultiply 変換 `straight_to_premultiplied`。`render_scene` が内部で構築・駆動し、crate 外へ出さない。
  - **［amend 2026-06-09］** 当初「公開 API は `render_scene` のみ」と書いたが、Render Host が surface 構築に必要とする補助関数を見落としていた。Painter/walk を crate 内部に閉じる意図はそのまま、surface 補助という第二の公開カテゴリを明文化する。実装は当初 `VelloPainter` / `TinySkiaPainter` を `pub use` していたが（外部利用なし）、本 amend で非公開化。なお ADR-0072 の「単一公開サーフェス」原則は Hayate の **外部** 契約（Element Layer proto）に関するもので、renderer crate ↔ adapter-web 間の **内部** crate API はその射程外。
- **Render Host の web surface（`VelloSurfaceHost` 等）は当面 adapter-web に残留**（H1）。native adapter 追加時に renderer crate へ移管を再検討（decisions-pending Open #2）。**［revisit: ADR-0068］** native が確定設計目標である以上、プラットフォーム非依存の Render Host 芯（policy/orchestration）と Font ロードを共有層へ **今 hoist** し、web 特有部分を `Surface` / `FontFetcher` trait の裏に置く。surface **生成**の web 実装が adapter-web に残る点（H1 の事実）は trait 実装として維持。

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
