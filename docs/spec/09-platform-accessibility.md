# §9 Platform Adapter & Accessibility

Platform Adapter の責務範囲と、アクセシビリティ（AccessKit）の所有・展開順序。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### PLAT-01 — Platform Adapter の責務は三つ
**規範文:** Platform Adapter の責務は IME 入力・クリップボード・raw 入力イベント変換の三つに限定する。Core は Platform Adapter を知らない。サーフェス生成とフレームタイミングは wgpu が担い、アクセシビリティ報告は AccessKit が担うため、いずれも Adapter の責務に含めない。
**出典:** ADR-0014
**状況:** ✅ — `adapters/web` は raw 入力変換（`on_pointer_*` / `on_wheel` / `on_key_down`）、IME（composition handlers）、clipboard（`element_paste`）に限定。Core は adapter 非依存。
**備考:** surface/present/resize は Render Host（§4 REND-14）、a11y は PLAT-03/04。

### PLAT-02 — クリップボードは element_paste
**規範文:** クリップボード貼り付けは `element_paste(id, text)` で `text-input` の `text_content` に反映する（active preedit があれば確定してから追記）。OS クリップボードの読み書きは host 側が担う。
**出典:** ADR-0014
**状況:** ✅ — `tree.rs:428` `element_paste()`、adapter `element_renderer.rs:563`、テスト `element_layer.rs:1118-1172`（空/追記/preedit確定/イベント発火/非text-input no-op）。
**備考:** —

### PLAT-03 — AccessKit は Core が所有
**規範文:** Core はアクセシビリティツリー（`accesskit::TreeUpdate`）の生成責務を持ち、`role` / `aria_label` 等の a11y データを Element に保持する。Platform Adapter は AccessKit のプラットフォーム実装を呼び OS の AT（UIA / NSAccessibility / AT-SPI / Web ARIA）に報告する。
**出典:** ADR-0041, `CONTEXT.md`「AccessKit」
**状況:** ✅ — `accesskit` 依存（`hayate-core`）；`ElementTree::accessibility_update()` が `layout_cache` 境界矩形 + `aria_label` / `role` から `TreeUpdate` を生成（`element/accessibility.rs`）；Canvas adapter `poll_accessibility()` が JSON 返却（`element_renderer.rs`）。
**備考:** Parley `LayoutAccessibility::build_nodes` による text run 詳細は将来。ネイティブ Platform Adapter への報告は PLAT-04。

### PLAT-04 — AccessKit 展開順序：ネイティブ優先
**規範文:** AccessKit 対応はネイティブ（UIA / NSAccessibility / AT-SPI）を優先し、Web Canvas Mode は Safari が EditContext API を正式サポートした時点で `accesskit-web`（不可視 ARIA DOM）を最優先で対応する。Web HTML Mode は実 DOM に ARIA 属性付与で対応する。
**出典:** ADR-0041
**状況:** 🟡 — 設計確定。Core `TreeUpdate` 生成は完了（PLAT-03）。ネイティブ AT 報告（UIA/NSAccessibility/AT-SPI）と Web Canvas `accesskit-web` は未着手。
**備考:** ネイティブ Platform Adapter crate が前提。

### PLAT-05 — surface / frame timing / a11y はアダプタ責務外（設計境界）
**規範文:** サーフェス生成・フレームタイミングは wgpu、アクセシビリティ報告は AccessKit が担い、Platform Adapter の責務に含めない。
**出典:** ADR-0014, `CONTEXT.md`「Platform Adapter」
**状況:** ✅ — 「やらないこと」の境界規範。surface は Render Host、a11y は Core+AccessKit に分離（PLAT-01/03 と整合）。
**備考:** —

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 4 | PLAT-01, PLAT-02, PLAT-03, PLAT-05 |
| 🟡部分 | 1 | PLAT-04 |
| ⬜未実装 | 0 | — |
