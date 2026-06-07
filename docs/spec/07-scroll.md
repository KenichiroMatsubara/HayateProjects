# §7 Scroll

`scroll-view` のオフセット管理と、Core / Platform Adapter の責務分離。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### SCR-01 — 基本 offset 積算は Core、物理演算は Platform Adapter
**規範文:** wheel delta の基本 offset 積算（nearest `scroll-view` 探索・content bounds への clamp）は hayate-core の Element Document Runtime が担う。慣性・rubber-band・snap 等の物理演算は Platform Adapter が担う。
**出典:** ADR-0046, ADR-0053（ともに ADR-0022 を supersede）
**状況:** ✅ — `tree.rs:815` `apply_wheel_delta()`（積算+clamp）、adapter `element_renderer.rs:444` `on_wheel()` が呼ぶ。clamp テスト `document_runtime.rs:252`。物理演算は adapter 責務として未実装（open）。
**備考:** [履歴 C-7.1] ADR-0022「scroll offset を上位層（Hayabusa）が所有」は superseded。`CONTEXT.md`「Scroll Offset」がまだ 0022 を参照する軽微な drift → 0046/0053 を正に更新要。

### SCR-02 — element_set_scroll_offset はプログラマティック専用
**規範文:** `element_set_scroll_offset(id, x, y)` はプログラム制御のスクロール専用 API として残す。基本 wheel 積算の経路には使わない。
**出典:** ADR-0046
**状況:** ✅ — `tree.rs:470` に実装。`apply_wheel_delta` が内部で commit に使用。
**備考:** —

### SCR-03 — scroll delivery はアプリ通知専用
**規範文:** `scroll` delivery イベントは parallax / lazy-load 等のアプリ通知専用であり、offset 積算目的には使わない。
**出典:** ADR-0046, ADR-0053
**状況:** ✅ — `renderer_event_state.rs` の `wheel()` が `Event::Scroll` を通知用に emit。積算は SCR-01 が別途担う。
**備考:** §6 EVT と整合（scroll は `adapterTier: deferred`）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 3 | SCR-01〜03 |
