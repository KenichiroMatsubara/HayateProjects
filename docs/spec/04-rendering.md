# §4 Raw Layer / Scene Graph / Rendering

保持型 Scene Graph、その単一 walk による描画、Scene Renderer の選択。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

## Scene Graph

### REND-01 — 保持型 Scene Graph と Node 種別
**規範文:** Hayate は保持型（retained）`SceneGraph` を持ち、Node 種別は `Rect` / `RoundedRing` / `TextRun` / `Image` / `Group` / `Clip` の GPU 直接処理可能な型のみとする。slotmap（generational arena）で NodeId を払い出す。
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
**規範文:** Z-Order は同一親内の兄弟順序で実現する。デフォルトは document order（後勝ち）、`StyleProp::ZIndex(n)` がその上書き。CSS stacking context は持たない。順序解決は `ElementTree::ordered_children(id)`（paint order = z 昇順・同 z は document 安定）の単一 seam に集約し、paint は前方反復、hit-test は `.rev()` で消費する（hit = paint の逆順を構造的に保証）。`resolved_elements` / HTML 経路は意図的にこの seam を通さず document order を保つ（CSS / AT が stacking・読み上げ順を担う）。
**出典:** ADR-0021, ADR-0060
**状況:** ✅ — `tree.rs` `ordered_children()` に集約。`scene_build` の2 sort site と hit-test の独立コンパレータを撤去。回帰テスト（tie-break 後勝ち / paint・hit 逆順一致）あり。
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
**状況:** ✅ — `backend/mod.rs` の `RenderHost`（`init_with_policy`、runtime 失敗時の `fallback_after_runtime_failure`）。
**備考:** web surface 初期化は当面 adapter-web に残留（ADR-0054 H1、decisions-pending Open #2 → §未決）。

### REND-09 — Renderer Selection Policy
**規範文:** どの renderer を許可しどの順で試すかは `Renderer Selection Policy` が決める。Vello を preferred default、tiny-skia を standard alternative とし、recording/null は非標準（診断）として分離する。各 backend の `name` / `try_init` / `try_init_sync_for_fallback` / `classify_init_error` は `SceneRendererKind` に集約し、`RenderHost` は policy の preference list を回すのみ。
**出典:** ADR-0050
**状況:** ✅ — `SceneRendererKind::{name, try_init, try_init_sync_for_fallback, classify_init_error}`（`backend/mod.rs`）；`RenderHost::init_with_policy` が preference list を反復。
**備考:** 新 backend 追加は enum variant + `SceneRendererKind` impl の1箇所 + backend crate。

### REND-10 — Vello を主候補 renderer とする
**規範文:** GPU 描画の主候補 renderer は Vello（Linebender, wgpu ベース）とし、`SceneGraph`→Vello Scene 変換は薄い独立 crate（`scene-renderers/vello`）に置く。公開 API は `render_scene` のみ。
**出典:** ADR-0006, ADR-0054
**状況:** ✅ — `scene-renderers/vello`（`VelloPainter` + `VelloSceneRenderer::render_scene`）。
**備考:** —

### REND-11 — tiny-skia を CPU フォールバックとする
**規範文:** Vello が使えない環境（GPU/WebGPU 不可）向けに tiny-skia を CPU レンダリングの Scene Renderer として持つ。feature-gate で1つだけ link し、二 WASM バイナリを維持する。
**出典:** ADR-0048
**状況:** ✅ — `scene-renderers/tiny-skia`（`TinySkiaPainter` + `render_scene`）。`backend-vello` / `backend-tiny-skia` feature。
**備考:** GPU バックエンドではなく CPU 代替（§1 CORE-02 と整合）。

---

## Raw Layer の公開

### REND-12 — Raw Layer は当面非公開（WIT 撤去）
**規範文:** Raw Layer（`Node` / `SceneGraph` の絶対座標直接制御）は Rust 内部実装に留め、外部公開契約（旧 Raw Layer WIT）は持たない。外部公開は Element Layer / §10 protocol contract に限る。
**出典:** ADR-0033 → ADR-0049（WIT 撤去で supersede）
**状況:** ⬜（公開なし、意図通り） — `Hayate/wit/` 不在。Raw Layer は Rust 内部 API のみ。
**備考:** [履歴] ADR-0033「Raw Layer WIT を world export から除外」は、WIT 自体の撤去（0049）で上書き。Raw Layer の外部公開は将来再検討。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 11 | REND-01〜11 |
| 🟡部分 | 0 | — |
| ⬜（非公開・意図通り） | 1 | REND-12 |
