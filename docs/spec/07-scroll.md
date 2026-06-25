# §7 Scroll

`scroll-view` のオフセット管理と、Core / Platform Adapter の責務分離。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### SCR-01 — 基本 offset 積算とスクロール物理は Core 所有、Platform Adapter はフレーム駆動と platform 識別供給に徹する
**規範文:** wheel/touch の基本 offset 積算（nearest `scroll-view` 探索・content bounds への clamp）に加え、**スクロール物理は hayate-core が所有する**。Core は二軸を持つ — **Scroll Gesture（意図分類）**: raw ポインタ列を「タップ / scroll」「掴んだ `scroll-view`」「適用すべき 1:1 follow デルタ」へ分類する純粋状態機械（slop 等 tunable は Adapter が供給）。**Scroll Physics Profile（感触）**: `auto` / `ios` / `android` の閉じた三値で、iOS 風（指数減衰＋sigmoid rubber-band）と Android 風（OverScroller の spline＋Material stretch）の**別アルゴリズムをいずれも Core が実装**し、`auto` は Adapter が渡す platform 識別から各 OS 相当へ解決する（Core 自身は platform を検出せず enum で解決＝platform-free を保つ）。Platform Adapter はフレーム駆動（毎フレーム Core の step を進める）・`Scroll Offset` 適用・ポインタ位置サンプリング・tunable/platform 識別の供給に徹する。現状は `auto` のみ公開（web で iOS profile に解決）、明示 `ios`/`android` 上書きの公開 API は将来。
**出典:** ADR-0113（スクロール物理を Core 所有とし ADR-0046 を supersede）、ADR-0046/0053（offset 積算は Core・ともに ADR-0022 を supersede）
**状況:** ✅ — `scroll.rs` の `ScrollGesture`（`is_drag_scroll_pointer` / `exceeds_slop` / `on_move` / `MoveOutcome`）＋ Scroll Physics 純粋関数（`ScrollPhysicsTuning`・`rubber_band_offset`・fling 指数減衰・damped spring ばね戻し、iOS profile 実装）。基本積算は `tree.rs` `apply_wheel_delta()`（積算+clamp）、clamp テスト `document_runtime.rs`。Android profile（spline/stretch）は Android タッチスクロール実装時に Core へ追加予定。
**備考:** [履歴] ADR-0046「物理演算は Platform Adapter 所有」は ADR-0113 が supersede（ジェスチャ認識の重複・別アルゴリズムの感触・プロファイル選択不能を解消）。`Scroll Offset` の基本積算を Element Document Runtime が単独所有する点・`scroll` イベントがアプリ通知専用な点は ADR-0046 から不変。[履歴 C-7.1] ADR-0022「scroll offset を上位層（Hayabusa）が所有」は superseded。

### SCR-02 — element_set_scroll_offset はプログラマティック専用
**規範文:** `element_set_scroll_offset(id, x, y)` はプログラム制御のスクロール専用 API として残す。基本 wheel 積算の経路には使わない。
**出典:** ADR-0046
**状況:** ✅ — `tree.rs:497` に実装。`apply_wheel_delta` が内部で commit に使用。
**備考:** —

### SCR-03 — scroll delivery はアプリ通知専用
**規範文:** `scroll` delivery イベントは parallax / lazy-load 等のアプリ通知専用であり、offset 積算目的には使わない。
**出典:** ADR-0046, ADR-0053
**状況:** ✅ — `ElementTree::on_wheel` が `Event::Scroll` を dispatch。積算は SCR-01 が別途担う。
**備考:** §6 EVT と整合（scroll は `adapterTier: deferred`）。

### SCR-04 — Scrollbar Chrome は core が overlay で描き Pointer Modality で分岐する
**規範文:** `scroll-view` の Scrollbar Chrome は core が overlay（レイアウト非予約）で描く。Pointer Modality で形態が分岐する — Mouse/Pen は Chromium をお手本にした操作可能なスクロールバー（thumb ドラッグ・track クリックで Scroll Offset を動かす）、Touch は Android-native をお手本にしたスクロール中のみ出る非操作の transient indicator。thumb ドラッグは Scroll Offset シーム（SCR-01 / `element_set_scroll_offset`）に収斂し、DOM 経路は native ドラッグが同じ Scroll Offset を生む（意味論パリティ）。content box 幅をレンダラー間で一致させるため DOM も overlay 固定（gutter 非予約）。
**操作意味論（Mouse/Pen, #409）:**
- **thumb ドラッグ** — thumb の pointer-down で掴み、pointer-move のトラック方向移動量を `max_offset / thumb_travel` 倍して Scroll Offset デルタに変換、wheel と同じ `apply_wheel_delta`（SCR-01）経由で commit する。よって thumb はトラック空間でポインタに 1:1 追従し、当該軸の端に達した残デルタは祖先 ScrollView へ chaining（ADR-0084）する。pointer-up / cancel でドラッグ終了。
- **track クリック（ページ送り）** — thumb の外側のトラック余白の press は、thumb の遠端より先なら forward、近端より手前なら back へ、`SCROLLBAR_PAGE_STEP`（名前付き定数・placeholder）1 段だけ `apply_wheel_delta` で送る。
- **収斂** — ドラッグ／クリックいずれも wheel と同じ Scroll Offset シーム（`element_set_scroll_offset`・ADR-0046）に収斂し、レンダラー方言を作らない。
- **modality 分岐** — 操作は Mouse/Pen のみ。Touch press は thumb を掴まずスルーする（transient indicator は非操作）。

**modality 分岐（最終ポインタ種別, #410）:** Scrollbar Chrome の形態は **selection chrome と同じ最終ポインタ種別**（`last_pointer_kind`・ADR-0104）で分岐し、scrollbar 用に追跡状態を増やさない。
- **Mouse/Pen** — 上記の操作可能スクロールバー（thumb＋track）を常時描く。
- **Touch transient indicator** — スクロール中だけ出てフェードする**非操作**の indicator。Touch modality で Scroll Offset が動くと当該 ScrollView の indicator を全可視で（再）点灯し、`SCROLLBAR_INDICATOR_HOLD_MS` の保持後 `SCROLLBAR_INDICATOR_FADE_MS` かけて線形にフェードアウトして消える。表示・フェードは時間駆動の `visual_dirty`（カーソル点滅・Transition と同じループ）に乗る（`ElementTree::advance_touch_scroll_indicators` が `render(now_ms)` で各 indicator の可視率を再計算し、生存中は box を visual-dirty に保ち、ゼロで破棄）。形は thumb 幾何を流用しつつ `SCROLLBAR_INDICATOR_THICKNESS`（thumb より細い）で箱の端に固定。**サム／トラックの hit-test 領域を持たない** — `begin_scrollbar_gesture` が Touch で早期 return し、indicator は内容フリックで動かす対象であって掴めるバーではない。静止 Touch 面はスクロールバーを一切描かない（モバイルに常駐バーは無い）。

**出典:** ADR-0110（ADR-0102 視覚お手本＝DOM／ADR-0104 modality 分岐 chrome を継承）
**状況:** 🟡 — 描画（#407）・Mouse/Pen 操作（#409）・Touch transient indicator 分岐（#410）実装済み。残るは DOM overlay 固定のみ。Canvas は overflow のある軸ごとに scrollbar overlay を ScrollView anchor 配下へ lowering する（`scene_build.rs` `emit_scrollbar_overlay`、`SCROLLBAR_*` 名前付き定数）。Mouse/Pen は操作可能 thumb、Touch は transient indicator（`emit_touch_scroll_indicator`・`SCROLLBAR_INDICATOR_*` 名前付き定数）。thumb／indicator 幾何は Scroll Offset と content size（`element_content_size`）に追従し、内容が収まる軸は描かない。ネスト時は内側 thumb が内側の箱に追従し外へ漏れない（#199/#200 と同座標系）。操作は paint と同一の幾何 seam（`scene_build::scrollbar_axes`）で hit-test し、`begin_scrollbar_gesture` / `drag_scrollbar`（`interaction.rs`）が `apply_wheel_delta` に載せる。回帰: core `scrollbar_overlay_scene.rs`（描画・DrawOp 列＋NodeKind walk）、core `scrollbar_drag.rs`（操作・thumb ドラッグ連続移動／track ページ送り／端での chaining／Touch スルー）、core `scrollbar_chrome_modality.rs`（modality 分岐・PointerKind パラメタ化＋Touch indicator の表示→フェード→消滅ライフサイクル）、tiny-skia `scrollbar_overlay_render.rs`（Pixmap）。**未実装（follow-up）:** DOM Renderer の overlay 固定（現状 `overflow: auto` の classic 予約 `dom-elements.ts:50`）。実値（thumb／indicator 寸法・色・フェード時間・`SCROLLBAR_PAGE_STEP` 等）は Chromium/Android 校正待ちの placeholder。
**備考:** [#391] 当初 by-design と整理しかけたが ADR-0102 が及ぶ既知ギャップと確定。chaining 意味論（SCR-01・ADR-0084）とは直交だが、thumb ドラッグの端処理は同じ `apply_wheel_delta` を再利用して意味論を一致させる。描画は #407、Mouse/Pen 操作は #409。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 3 | SCR-01〜03 |
| 🟡部分 | 1 | SCR-04（描画＋Mouse/Pen 操作＋Touch transient indicator 済み・DOM overlay 固定のみ open・ADR-0110） |
