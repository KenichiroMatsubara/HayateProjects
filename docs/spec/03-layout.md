# §3 Layout

レイアウト計算（Taffy）と、Element Layer / Raw Layer の二層構成における位置づけ。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### LAY-01 — Taffy をレイアウトエンジンとする
**規範文:** レイアウトエンジンは Taffy（Pure Rust の Flexbox / CSS Grid / Block）を採用し、C++ ビルドチェーンを混入させない。`crates/vendor/taffy` に vendoring 済み。
**出典:** ADR-0004, ADR-0007
**状況:** ✅ — `taffy_bridge.rs`（StyleProp→`taffy::Style` 変換）、`layout_pass.rs`（`TaffyTree<MeasureCtx>` 保持）、`element_layer.rs` テストで layout 経路を検証。
**備考:** —

### LAY-02 — Layout は Element Layer の必須構成要素
**規範文:** Taffy レイアウトは Element Layer の不可分な一部として `ElementTree` に内包し（`tree.layout: LayoutPass`）、独立 crate には分離しない。layout 計算は `render()` に統合する。
**出典:** ADR-0008（「optional module」案を supersede）
**状況:** ✅ — `tree.rs` の `ElementTree` が `LayoutPass` を embed。feature gate なし。
**備考:** [履歴] ADR-0008 旧案「layout を後付け optional module」は廃止。「layout 不要」需要は §3 LAY-03 の二層分離で解決。

### LAY-03 — Element / Raw の内部二層分離（Raw は layout 非依存・外部非公開）
**規範文:** Element Layer（layout 統合）と Raw Layer（`SceneGraph` + `Node`・layout 非依存）は**内部の**二層として分離し、描画パイプライン（`render_scene_graph`）のみ共有する。Raw Layer はレイアウトを経由しない内部 lowering target であり、**外部公開しない**（公開契約は ADR-0072 で棄却）。
**出典:** ADR-0008（内部 layer 二層化）、ADR-0072（外部公開の棄却）
**状況:** ✅ — 構造分離は実装済み（`node.rs` の `SceneGraph::insert`/`insert_child` が public（crate 内）、Element に依存しない）。外部公開契約は持たない＝規範どおり（ADR-0072 で確定。旧「公開契約は未整備」TODO を解消）。
**備考:** 外部公開は Element Layer ベース proto 契約の一つだけ（§4 REND-12）。layout-free な game HUD / Infinite Canvas 公開は提供しない。

### LAY-04 — Taffy は ElementTree から lazy に派生する投影
**規範文:** `ElementTree` が構造とデータの唯一の owner。Taffy ツリーはその block-box 部分集合の derived projection（inline text element 除外・IFC root は measured leaf）であり peer ではない。構造 mutation は `ElementTree` と structure-dirty 集合のみ触り、Taffy には触れない。Taffy 投影は `LayoutPass::run` 冒頭で dirty-scoped に reconcile する（`TaffyProjection` seam が `TaffyTree` と ElementId↔NodeId マップを所有）。`el.taffy_node` は `Option`（inline text element は None）。Taffy-as-owner（`Display::None` で inline text element を載せる案）は不採用。
**出典:** ADR-0064（ADR-0004/0008 を継承、ADR-0063 が引き金）
**状況:** ✅ — `taffy_projection.rs` が `TaffyTree` + ElementId↔NodeId マップを所有。構造 mutation（`element_create`/`append_child`/`insert_before`/`remove`）は `structure_dirty` 記録のみ。`LayoutPass::run` 冒頭で `projection.reconcile()`。inline text element は `taffy_node: None`、IFC root は measured leaf。
**備考:** 2b（ADR-0063）が Taffy を 1:1 ミラーから非自明投影（inline text element 除外・IFC leaf・reparent クラス反転）に変えたことが本項目の引き金。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 4 | LAY-01, LAY-02, LAY-03, LAY-04 |
| 🟡部分 | 0 | — |
| ⬜未実装 | 0 | — |
