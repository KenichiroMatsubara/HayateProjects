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

### LAY-03 — Raw Layer は Layout なしで使用可能
**規範文:** Raw Layer（`SceneGraph` + `Node` の絶対座標直接指定）はレイアウト計算を経由せず使用できる。Element Layer（layout 統合）と Raw Layer（layout 非依存）は二層として分離し、描画パイプライン（`render_scene_graph`）のみ共有する。
**出典:** ADR-0008（layer 二層化）
**状況:** 🟡 — 構造分離は実装済み（`node.rs` の `SceneGraph::insert`/`insert_child` が public、Element に依存しない）。ただし Raw Layer を layout なしで使う実利用例・公開契約（§4 REND-16/17 で WIT は撤去済み）は未整備。
**備考:** Raw Layer の外部公開は §4 参照（現状は Rust 内部のみ）。

### LAY-04 — Taffy は ElementTree から lazy に派生する投影
**規範文:** `ElementTree` が構造とデータの唯一の owner。Taffy ツリーはその block-box 部分集合の derived projection（inline span 除外・IFC root は measured leaf）であり peer ではない。構造 mutation は `ElementTree` と structure-dirty 集合のみ触り、Taffy には触れない。Taffy 投影は `LayoutPass::run` 冒頭で dirty-scoped に reconcile する（`TaffyProjection` seam が `TaffyTree` と ElementId↔NodeId マップを所有）。`el.taffy_node` は `Option`（span は None）。Taffy-as-owner（`Display::None` で span を載せる案）は不採用。
**出典:** ADR-0064（ADR-0004/0008 を継承、ADR-0063 が引き金）
**状況:** ⬜未実装 — 設計確定。現状は eager で各 mutation site が inline `taffy.add_child`（`tree.rs:602`）し、全 element に Taffy leaf を確保（`:188`）。`TaffyProjection` 抽出・structure-dirty 集合・lazy reconcile・inline taffy 呼び出し撤去が残タスク。RN(Fiber↔Yoga)・Flutter(Element↔RenderObject) と同形。
**備考:** 2b（ADR-0063）が Taffy を 1:1 ミラーから非自明投影（span 除外・IFC leaf・reparent クラス反転）に変えたことが本項目の引き金。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 2 | LAY-01, LAY-02 |
| 🟡部分 | 1 | LAY-03 |
| ⬜未実装 | 1 | LAY-04（Taffy lazy 投影、ADR-0064） |
