# §4 Raw Layer / Scene Graph / Rendering

保持型 Scene Graph、その単一 walk による描画、Scene Renderer の選択。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

## Scene Graph

### REND-01 — 保持型 Scene Graph と Node 種別
**規範文:** Hayate は保持型（retained）`SceneGraph` を持ち、Node 種別は `Rect` / `RoundedRing` / `TextRun` / `Image` / `Group` / `Clip` に加え、element retained identity 用の `ElementAnchor` を持つ。slotmap（generational arena）で NodeId を払い出す。lowering は dirty-gated incremental（ADR-0086）。
**出典:** ADR-0006, ADR-0054
**状況:** ✅ — `node.rs` の `NodeKind`、`SceneGraph { nodes: SlotMap, roots: Vec }`。
**備考:** —

### REND-02 — Transform は Group Node
**規範文:** transform は `NodeKind::Group { transform: [f64;6] }` として表現し、`StyleProp::Transform` として座標に焼き込まない。Group は Vello の `push_transform`/`pop` に対応し、layout 再計算ゼロでサブツリーを変換できる。
**出典:** ADR-0020
**状況:** ✅ — `node.rs` の `Group`、`painter.rs:329` で walk が push/pop_transform に変換、`scene_build.rs:88` が transform を持つ要素を Group でラップ。
**備考:** —

---

## Z-Order

### REND-03 — Z-Order は子順序（stacking context なし）
**規範文:** Z-Order は同一親内の兄弟順序で実現する。デフォルトは document order（後勝ち）、effective `z-index` がその上書き。CSS stacking context は持たない。順序解決は Element Document Runtime の retained `PaintOrder`（paint order = z 昇順・同 z は document 安定）の単一 seam に集約し、scene lowering と Layer Topology は forward view、hit-test は同じ view の `.rev()` を消費する（hit = paint の逆順を構造的に保証）。`resolved_elements` / HTML / accessibility 経路は意図的にこの seam を通さず document order を保つ（CSS / AT が stacking・読み上げ順を担う）。
**出典:** ADR-0021, ADR-0060
**状況:** ✅ — `element/paint_order.rs` が parent 単位の順序を保持し、structure / reparent / effective z-index 変更後の最初の consumer だけが再構築する。scene lowering・Layer Topology・hit-test の独立 sort を撤去し、steady-state hit-test は allocation / sort なし。回帰テスト（nested / tie-break / reparent / hidden / scroll / HTML・accessibility document order）あり。
**備考:** [解決 C-4.1] 旧・順序3分散（paint昇順 / hit-test降順 / resolved無ソート）を単一 seam に統一。`walk_resolved` の document-order は ADR-0060 で意図的に seam 対象外（再ソート禁止）。

---

## 描画パイプライン（ADR-0054）

### REND-04 — 単一 walk（render_scene_graph）
**規範文:** `SceneGraph` の walk は `hayate-core::render` の `render_scene_graph(graph, painter)` 1箇所に集約する。backend は walk を実装しない。
**出典:** ADR-0054
**状況:** ✅ — `render/painter.rs:281` `render_scene_graph` + `walk_node`（内部）。vello/tiny-skia painter に walk 重複なし。
**備考:** ADR-0054 が ADR-0050 の「walk の所在未定」ギャップを閉じた、設計書の手本となる項目。

### REND-05 — ScenePainter trait（backend は実装のみ）
**規範文:** backend は `ScenePainter` trait（`fill_rect` / `draw_text_run` / `draw_image` / `push_transform`/`pop_transform` / `push_clip_rect`/`pop_clip`）を実装するだけとする。Group/Clip は対で呼ぶ。`ScenePainter` は core 内部 seam であり host 向け契約ではない。
**出典:** ADR-0054
**状況:** ✅ — `painter.rs` の `ScenePainter`。`VelloPainter`/`TinySkiaPainter` は trait 実装のみ、adapter backend は `render_scene` を呼ぶだけ。
**備考:** 明示 stack API（RAII 不採用）は Vello sub-scene の lifetime と両立させるため。

### REND-06 — RecordingPainter / NullPainter（診断・テスト）
**規範文:** 診断・テスト用に `RecordingPainter`（flat `Vec<DrawOp>` に materialize）と `NullPainter`（no-op）を core に置く。SceneGraph clone 型の `RecordingBackend`/`NullBackend` は持たない。
**出典:** ADR-0054
**状況:** ✅ — `painter.rs` の `RecordingPainter`/`NullPainter`/`SceneRecorder`。pure Rust テスト（Group/Clip/z-order）の駆動に使用。
**備考:** —

---

## Scene Renderer 選択（ADR-0050）

### REND-07 — Backend と Scene Renderer の分離
**規範文:** GPU API を指す `Backend`（= wgpu）と、`SceneGraph` を消費する `Scene Renderer`（Vello / tiny-skia）を分離する。Core は独自 Backend 抽象を持たない。
**出典:** ADR-0050
**状況:** ✅ — `backend/mod.rs` の `SceneRenderer` trait（`render_scene`/`clear`/`resize`）。汎用 GPU 抽象 trait は不在（wgpu 直）。
**備考:** —

### REND-08 — Render Host（surface / 選択 / fallback）
**規範文:** `Render Host` が renderer の surface 初期化・present・resize・選択・資源寿命管理・一方向 fallback を担う。
**出典:** ADR-0050, ADR-0054
**状況:** ✅ — Render Host 芯は `crates/app-host/src/render_host.rs` へ hoist 済み（ADR-0132）: `RenderHost::init_with_policy`（policy の試行順で init）＋ runtime 失敗時の `fallback_after_runtime_failure`（一方向）。web（`backend/mod.rs` の `WebRendererInit` / `WebCanvasSurface`）と desktop（`crates/platform/desktop` の `DesktopRendererInit` / `DesktopWindowSurface`、issue #801）の2アダプタが同一の芯を駆動する。Android 結線は #802。
**備考:** ADR-0054 H1「surface は adapter-web 残留」は ADR-0068 が revisit — surface **生成**は `impl Surface`（web: canvas / desktop: winit window）として platform に残り Host 芯は共有。`classify_init_error` は adapter 個別実装のまま（#672）。

### REND-09 — Renderer Selection Policy
**規範文:** どの renderer を許可しどの順で試すかは `Renderer Selection Policy` が決める。Vello を preferred default、tiny-skia を standard alternative とし、recording/null は非標準（診断）として分離する。各 backend の `name` / `try_init` / `try_init_sync_for_fallback` / `classify_init_error` は `SceneRendererKind` に集約し、`RenderHost` は policy の preference list を回すのみ。
**出典:** ADR-0050
**状況:** ✅ — `SceneRendererKind::{name, try_init, try_init_sync_for_fallback, classify_init_error}`（`backend/mod.rs`）；`RenderHost::init_with_policy` が preference list を反復。
**備考:** 新 backend 追加は enum variant + `SceneRendererKind` impl の1箇所 + backend crate。[更新 2026-07-18] web もネイティブ（REND-15）と同じくランタイム上書きを持つ：`?renderer=vello|tiny-skia`（`SceneRendererKind::name()` と同一語彙）を `@torimi/hayate-host` の `resolveCanvasBackendSelection` が deep-link として honor し、選択 renderer と選択理由を console へ出す（Rust 側 `render_host.rs` の `selected scene renderer:` / `scene renderer rejected:` ログが console_log 経由でブラウザに届く）。web は「1バイナリ1レンダラ」排他（REND-11）なので上書きはロードする WASM バンドルの選択として効く。

### REND-10 — Vello を主候補 renderer とする
**規範文:** GPU 描画の主候補 renderer は Vello（Linebender, wgpu ベース）とし、`SceneGraph`→Vello Scene 変換は薄い独立 crate（`scene-renderers/vello`）に置く。公開 API は `render_scene` のみ。
**出典:** ADR-0006, ADR-0054
**状況:** ✅ — `scene-renderers/vello`。公開 API は `VelloSceneRenderer::render_scene` と surface 補助（`VelloRenderTarget`/`create_target_view`/`create_blitter`、Render Host が使用）。`VelloPainter`（`ScenePainter` 実装＝walk コールバック）は crate 内部。
**備考:** [訂正 2026-06-09] 旧実装は `VelloPainter`/`TinySkiaPainter` を `pub use` 公開していたが（外部利用なし、ADR-0054「walk/Painter は内部」違反）非公開化。ADR-0054 を amend し「公開 API＝`render_scene` + surface 補助」を明文化。

### REND-11 — tiny-skia を web 専用 CPU フォールバックとする
**規範文:** tiny-skia は **web 専用**の最終 CPU フォールバック Scene Renderer とする。Auto は WebGPU が使えるとき Vello → tiny-skia の順で初回 boot 候補を試し、WebGPU が使えない環境では tiny-skia を直接選ぶ。ネイティブの代替経路は skia-safe（REND-14/15）が担い、tiny-skia をネイティブに結線しない。
**出典:** ADR-0048, ADR-0146
**状況:** ✅ — `scene-renderers/tiny-skia`（`TinySkiaPainter` + `render_scene`）。`backend-vello` / `backend-tiny-skia` feature。実装は従来から web のみで、住み分けと整合。
**備考:** GPU バックエンドではなく CPU 代替（§1 CORE-02 と整合）。[更新 2026-07-10] ADR-0146 がネイティブの standard alternative を skia-safe と定め、tiny-skia の守備範囲を web 専用へ明文化（旧規範文は環境を限定していなかった）。

---

## Raw Layer の公開

### REND-12 — Raw Layer は外部非公開（確定）
**規範文:** Raw Layer（`Node` / `SceneGraph` の絶対座標直接制御）は Rust 内部 lowering target に留め、**外部公開契約を持たない（確定棄却）**。Hayate の外部公開は Element Layer / §10 protocol contract の一つだけ。
**出典:** ADR-0072（公開 Raw Layer を正式棄却。ADR-0033→0049 を継承）
**状況:** ✅ — `Hayate/wit/` 不在、Raw Layer は Rust 内部 API のみ＝規範どおり。ADR-0072 で「将来再検討」を閉じ確定。
**備考:** [履歴] ADR-0013 の二層**公開**は ADR-0049（WIT 撤去）で機構が消え、ADR-0072 が公開意図を正式棄却。内部の二層分離（Element→Raw lowering）は §3 LAY-03 で維持。将来 layout-free 公開が真に必要になれば ADR-0072 を supersede して reopen。

---

## App Host（mount 先・boot シーム）

### REND-13 — App Host の boot シーム（`tick` ＋ `request_redraw` ＋ push 型 DeliverySink）
**規範文:** プラットフォーム非依存の共有層 **App Host**（tree 実体所有・フレームループ・Event Delivery drain・Font ロードを担い、内部で Render Host を駆動する mount 先）は、consumer（in-process Rust の Hayabusa / wire 経路の Tsubame Canvas Renderer）と platform を **consumer 非依存の2 seam** で繋ぐ。**(1) フレームループ**: OS フレームループ（web `requestAnimationFrame` / Android `Choreographer` / desktop winit event loop）は Platform Front が所有する。App Host は `tick(timestamp_ms)` を公開し（per-consumer なフレームコールバック trait は持たない）、構築時注入の単一 `request_redraw: impl Fn()` クロージャが唯一の wake 入口となる。idle からの wake 源は三つ（継続＝`tick` 末尾に App Host が／入力到着＝Platform Adapter が／非同期 signal 変化＝consumer が呼ぶ）で、いずれも同一 `request_redraw` を叩く。pending が無ければ idle へ落ちる（毎フレーム回し続けない）。**(2) Event Delivery**: drain は App Host、handler map は consumer 所有。App Host は push 型 `DeliverySink` を**毎フレーム無条件に**呼び（空 batch でも）、consumer が delivery handler 実行 ＋ reactive graph flush を行う。`tick` のフェーズ順は drain → advance（handler＋flush・唯一の flush 点）→ `commit_frame`（layout settling）→ render（`render_scene_graph`→Render Host→`Surface::present`）。
**出典:** ADR-0117（app-host-boot-seam）、ADR-0068（共有層 hoist の継続）
**状況:** ✅ — `crates/app-host/src/lib.rs` の `AppHost<S: Surface>`（`new(surface, request_redraw)` / `tick(timestamp_ms)` / `mount(DeliverySink)` / `tree_mut`）、`DeliverySink` trait。`tick` は drain（`poll_deliveries`）→ DeliverySink 無条件 push → `ElementTree::render`（内部 `commit_frame`）→ present → pending 残存時に `request_redraw` 再要求。回帰テスト `tick_drains_delivery_and_sink_mutates_tree`。
**備考:** Render Host（REND-08・surface/present/fallback）の上位に立つ orchestration 層。consumer 固有知識（Hayabusa の handler map / Tsubame の wire projection）を App Host に持ち込まない設計が同一 App Host への両 mount を成立させる。

---

## ネイティブ Scene Renderer 戦略（skia-safe、ADR-0146）

### REND-14 — skia-safe Scene Renderer（ネイティブ専用）
**規範文:** ネイティブ（desktop + Android）は Google Skia（skia-safe バインディング）による Scene Renderer を `scene-renderers/skia` crate として持つ。crate は `ScenePainter` と `LayerRasterizer`/`LayerCompositor`（ADR-0125。キャッシュ面 = SkSurface、合成 = drawImage）の実装＋per-renderer golden だけを持ち、walk・planning は共有実装のまま（REND-04/05 維持）。painter は **surface 非依存**（渡された Skia Canvas に描くだけ）とし、surface は CPU raster を導入形、Android は Ganesh GL（EGL）surface を早期フォローアップとして platform adapter 側に置く（EGL 管理を core に持ち込まない = REND-07 維持。Skia Vulkan / Graphite は棄却）。テキストはレイアウト正本を parley のまま、確定済みグリフ ID・位置を SkTextBlob でラスタライズのみ行う（SkParagraph / SkShaper 不使用。SkTypeface は fontique と同一のフォントバイト列から生成）。`paints_color_glyphs()` = true（Vello 以外で初のカラーグリフ対応）。wasm32 は対象外。ビルド供給は crates.io ＋ビルド済みバイナリの厳密ピン（ソースベンダリングしない、ADR-0007 例外）。
**出典:** ADR-0146（PRD #798）、ADR-0125、ADR-0007
**状況:** ✅ — crate 新設（#800: `crates/scene-renderers/skia`、surface 非依存 painter・SkTextBlob・LayerRasterizer/LayerCompositor・per-renderer golden）、desktop 結線（#801: CPU raster + softbuffer present、`SceneRendererKind::Skia` の `paints_color_glyphs()` = true / `requires_webgpu()` = false）、Android raster 結線（#802: `ANativeWindow_lock`/`unlockAndPost` への CPU present、`crates/platform/mobile/android/src/skia_window.rs`・`skia_present.rs`、カラー絵文字は CBDT ビットマップ経路で実機確認済み）、Android GL（#803: Ganesh GL/EGL surface `crates/platform/mobile/android/src/skia_gl_window.rs`——EGL 管理はアダプタ封じ込め（REND-07 維持）、painter は無変更のまま Canvas の出自が FBO0 wrap（`gpu::surfaces::wrap_backend_render_target`）へ替わるだけ。OPPO A101OP（Adreno 620）実機で GPU 描画・EGL/GL 情報 logcat・raster との出力一致を確認済み）まで実装済み。品質実測に基づく既定確定は後続の完全人力 issue。
**備考:** golden は per-renderer 方式（自分の過去出力との回帰のみ・クロスレンダラのピクセル比較なし）＋共有 demo-fixtures。CI golden は Linux desktop raster 経路のみ、Android GL は完全人力・実機確認 issue で担保。

### REND-15 — ネイティブ Renderer Selection Policy（vello → skia）
**規範文:** Renderer Selection Policy（REND-09）をネイティブにも通す。既定は vello を preferred default、skia をネイティブの standard alternative とする**一方向 fallback**（vello 初期化失敗 → skia）。両レンダラを常時リンクし、ランタイム上書き（Android: intent extra / desktop: env・CLI フラグ）で強制指定できる（ADR-0138/0140/0145 の「常時コンパイル＋ランタイムフラグ」流儀。web の「1バイナリ1レンダラ」排他はネイティブでは採らない）。選択されたレンダラ・選択理由（`RendererSelectionReason`）・GL 時は EGL/GPU 情報を logcat / stderr に記録する。`backend-vello` feature（default on）で vello/wgpu をネイティブビルドから外せる出口を持つ。
**出典:** ADR-0146（PRD #798）、ADR-0050
**状況:** ✅ — desktop は実装済み（#801）: 前提工事の Render Host 芯導入（REND-08）を経て、`native_renderer_selection_policy`（`crates/app-host/src/renderer_selection.rs`、vello → skia の一方向 fallback・強制指定の DisabledByPolicy 観測）を desktop Platform Front が実行する。強制切替は env `HAYATE_RENDERER` / CLI `--renderer`（`crates/platform/desktop/src/renderer_config.rs` の名前付き定数、CLI 優先）、`backend-vello` feature（default on）off で vello/wgpu を含まず skia 単独起動。選択・却下は `RendererSelectionReason` 語彙で stderr ログ。Android も実装済み（#802）: 同じ `native_renderer_selection_policy` を Android アダプタが実行し、intent extra `hayate.renderer`（`vello`/`skia`、`crates/platform/mobile/android/src/renderer_config.rs` の名前付き定数）で APK 再ビルドなしに強制切替、選択・却下・fallback は logcat で観測可能（OPPO 実機確認済み）。skia 内 raster/GL の切替も実装済み（#803）: intent extra `hayate.skia_surface`（`raster`/`gl`、既定は名前付き定数 `DEFAULT_SKIA_SURFACE` = `gl`——確定値は後続の完全人力 issue）で切替、GL（EGL）初期化失敗時は理由を logcat に残して skia raster へ一方向 fallback（boot は落ちない）、GL 選択時は EGL vendor / GL renderer 文字列を logcat に記録（OPPO 実機確認済み）。
**備考:** skia を preferred default へ昇格するかは実測後の別 ADR。web の selection policy と同一の観測可能な語彙で採否を追う。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 15 | REND-01〜15 |
