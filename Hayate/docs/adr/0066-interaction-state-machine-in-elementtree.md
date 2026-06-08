# Interaction 状態機械を ElementTree に置く（ADR-0053 を完遂）

**Status: accepted（ADR-0053 を完遂。ADR-0056/0032/0031/0018 と整合）**

**Date: 2026-06-07**

## Context

ADR-0053 は Element Document Runtime を hayate-core に置き、listener 登録・bubble dispatch・基本 scroll・focus を担わせた。ADR-0056 は擬似スタイル解決（`:hover`/`:active`/`:focus`）を runtime に足した。しかし **入力→セマンティックイベントの状態機械は依然 Platform Adapter（`hayate-adapter-web` の `RendererEventState`）に残り、tree が持つ状態と二重化している**。

実体（`renderer_event_state.rs` / `element_renderer.rs`）:

- `RendererEventState.active_element`（:27）↔ `tree.active_element`（`set_active_element`、:115/133 で両方書く）。**二重**。
- `RendererEventState.focused_element`（:30）↔ `tree.focused_element`（`element_renderer.rs:304–318` の old/new 手動 diff で同期）。**二重**。
- hover は既に tree のみ（`update_pointer_hover`）。
- 状態機械はイベント（Click/ActiveStart/Focus/Blur/HoverEnter/Leave/…）を生成し tree 経由で dispatch するが、状態を**両所**に書く。

tree が focus/active を要るのは描画正本だから — カーソル点滅（ADR-0032）、`:active`/`:focus`/`:hover` 擬似スタイル解決（ADR-0056）。そして「pointer down → click/active/focus」「move → hover」「up → active-end」「key → focused へ」は**プラットフォーム非依存の document セマンティクス**。CONTEXT/ADR-0053 は「Platform Adapter は raw 入力の変換と描画 flush のみ」と定めており、**状態機械は本来 core にあるべきで、adapter に漏れている**。

## Decision

**Interaction 状態機械を `ElementTree` に丸ごと移す。** `ElementTree` が入力ハンドリング surface を持つ:

```
ElementTree::on_pointer_down(x, y)      // hit_test + 状態遷移 + セマンティックイベント生成/dispatch
ElementTree::on_pointer_up(x, y)
ElementTree::on_pointer_move(x, y)      // has_layout guard 内包、hover は tree
ElementTree::on_wheel(x, y, dx, dy)
ElementTree::on_key_down(key, modifiers)// tree.focused へ
ElementTree::on_text_input / on_composition_start/update/end
```

- **focus/active/hover を `ElementTree` が単独所有。** `RendererEventState` の二重フィールド（`active_element`/`focused_element`）と `element_renderer.rs:304–318` の手動 focus 同期を**削除**。
- `tree: Option<&mut ElementTree>` の全引き回し（テスト fallback `raw_events`）を**削除** — テストは `ElementTree` を直接叩き `poll_deliveries()` で検証（interface = test surface）。
- `last_pointer_pos`（sub-pixel move dedup）は PointerMove emission を gate するセマンティクスなので tree へ移す（移動 coalescing をプラットフォーム横断で統一）。
- **Platform Adapter は薄い翻訳層に**：raw platform/JS 入力（pointer/key/wheel/EditContext）→ `tree.on_*` 呼び出し ＋ surface/render flush のみ。

## Consequences

- interaction 状態の owner が1つ（locality）。focus/active の二重化と手動同期が消え、silent な focus ずれの面が消える。
- テストが `ElementTree` を直接叩ける（`raw_events` fallback 削除、testability 向上）。
- `element_renderer.rs`（1593行）が **event-routing 責務を脱ぐ** → god-module 分割（候補 D2: EventDispatcher を core が吸収）を素直にする。
- Web/native adapter が**同一の core 状態機械**を共有（セマンティクスがプラットフォーム横断で一致）。
- ADR-0053（runtime が dispatch/focus を持つ）を完遂。ADR-0056（擬似スタイルは tree の hover/active/focus を読む）・ADR-0032（カーソル点滅は tree.focused）・ADR-0031（セマンティック名）・ADR-0018（poll・callback なし）と整合。

## Considered Options

- **`RendererEventState` を入力層として残し、二重フィールドだけ撤去して tree state を read/write**: 却下。`RendererEventState` が状態を持たない薄い wrapper になり shallow。状態機械ロジックは document セマンティクスで core に属す。`Option<tree>` 引き回しも残る。
- **現状維持**: focus/active 二重・手動同期・silent バグ面。

## 関係

- ADR-0053: 完遂（入力→イベントの状態機械を runtime に収める）。
- ADR-0056/0032/0031/0018: 整合（tree の hover/active/focus を各機能が読む）。
- 候補 D2（`element_renderer.rs` god-module 分割）: 本決定が event-routing を剥がし前提を整える。
