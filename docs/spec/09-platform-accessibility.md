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

### PLAT-06 — Android を最初のネイティブ Platform Adapter とする（winit 不採用）
**規範文:** Android を最初のネイティブ Platform Adapter ターゲットとする（iOS は後続・本ラウンド範囲外）。段階スコープは (A) 描画スモークテスト（`hayate-adapter-android` crate + example、wgpu/Vulkan surface、入力/IME/AccessKit なし）→ (B) タッチ入力を Element Document Runtime に接続 → (C) `hayate-adapter-web` 同等のフルパリティ（IME ブリッジ・AccessKit・クリップボード）。Platform Adapter は `winit` 等の汎用ウィンドウ抽象を使わず、各プラットフォームのネイティブ API（Android は `android-activity`）に直接バインドする。stage C のビルド基盤は GameActivity（`android-activity` の `game-activity` バックエンド）+ Gradle とし、IME はソフトキーボードの `InputConnection` を GameTextInput 経由で native に上げる（ADR-0094、cargo-apk/native-activity から移行）。`hayate-core` はどのプラットフォーム依存も持たない。ADR-0051（Tsubame 優先）と並行トラックであり supersede ではない。
**出典:** ADR-0087, ADR-0094
**状況:** 🟡 — `crates/adapters/android`（`lib.rs` / `surface_lifecycle.rs` / `touch_input.rs` / `scene_demo.rs` / `app.rs`、`tests/apk_packaging.rs`）が存在。(A) 描画スモーク完了。(B) タッチ入力に加え、ループが `tree.render()` で `ElementTree`→`SceneGraph` を lowering して毎フレーム present するようになり（`viewport_for_surface` で viewport を surface px に追従、`scene_demo` の `:active` ボタンでタップが画面に反映）、タッチが描画されないツリーを駆動していた穴を解消。(C) フルパリティ（IME / AccessKit / clipboard）は未着手だが、パッケージング基盤を GameActivity + Gradle へ移行（`android-app/` の Gradle プロジェクト + `MainActivity : GameActivity` + Manifest、`Cargo.toml` を `game-activity` feature へ、cargo-apk metadata 撤去、ADR-0094）。NDK/SDK/Gradle 不在環境では host テスト可能な純粋 seam（`surface_lifecycle` / `touch_input` / `scene_demo`）と packaging 契約テスト（`tests/apk_packaging.rs` が Gradle/Manifest/Kotlin を読む）のみ検証可能で、`app.rs` の NDK glue・Gradle ビルドは実機/エミュレータ検証（#195）が必要。
**備考:** アダプタ間でウィンドウ/イベントループの共有コードは持たない（各アダプタが lifecycle/surface を再実装）。PLAT-04 のネイティブ AccessKit 報告は本アダプタを前提とする。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 4 | PLAT-01, PLAT-02, PLAT-03, PLAT-05 |
| 🟡部分 | 2 | PLAT-04, PLAT-06 |
| ⬜未実装 | 0 | — |
