# §7 Scroll

`scroll-view` のオフセット管理と、Core / Platform Adapter の責務分離。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### SCR-01 — 基本 offset 積算は Core、物理演算は Platform Adapter
**規範文:** wheel delta の基本 offset 積算（nearest `scroll-view` 探索・content bounds への clamp）は hayate-core の Element Document Runtime が担う。慣性・rubber-band・snap 等の物理演算は Platform Adapter が担う。
**出典:** ADR-0046, ADR-0053（ともに ADR-0022 を supersede）
**状況:** ✅ — `tree.rs:832` `apply_wheel_delta()`（積算+clamp）、adapter `element_renderer.rs:323` `on_wheel()` が呼ぶ。clamp テスト `document_runtime.rs:252`。物理演算は adapter 責務として未実装（open）。
**備考:** [履歴 C-7.1・解決] ADR-0022「scroll offset を上位層（Hayabusa）が所有」は superseded。`CONTEXT.md`「Scroll Offset」の旧 0022 参照（物理演算を上位層が持つ）は 0046/0053 に更新済み（2026-06-09）。

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
**出典:** ADR-0108（ADR-0102 視覚お手本＝DOM／ADR-0104 modality 分岐 chrome を継承）
**状況:** ⬜ — 未実装。Canvas は現状 `Clip` + scroll-offset `Group` のみ lowering（`scene_build.rs:764`）でスクロールバー Node を発行しない。DOM Renderer は `overflow: auto` の native UA chrome（`dom-elements.ts:50`）で classic 予約。両者を overlay へ寄せる実装は open。
**備考:** [#391] 当初 by-design と整理しかけたが ADR-0102 が及ぶ既知ギャップと確定。chaining 意味論（SCR-01・ADR-0084）とは直交。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 3 | SCR-01〜03 |
| ⬜未実装 | 1 | SCR-04（Scrollbar Chrome・ADR-0108） |
