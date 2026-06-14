# 自己配線した DOM 入力イベントは headless browser の wasm-bindgen-test で検証する

**Status: accepted**

**Date: 2026-06-13**

## Context

ADR-0080 / ADR-0082 により、canvas の raw 入力イベント（`pointerdown`/`pointermove`/`pointerup`/`pointercancel`/`pointerleave` + `wheel`）の購読は Tsubame host（`init.ts` の `attachPointerInput`）から Platform Adapter（`hayate-adapter-web`, `web-sys`）の自己配線へ移行した。リスナーは `Closure` で canvas に登録され、座標変換した入力を `pending_pointer` バッファへ enqueue し、`render()` 冒頭の drain で Core の `on_pointer_*` へ適用される。

この経路には native `#[test]` では到達できない部分がある：

- 純粋ロジック（`to_canvas_coords` 座標変換、`coalesce_pointer_inputs` の 1px dedup、`final_anchor`）は `pointer_input.rs` の `#[cfg(test)]` で全ターゲット検証済み。
- Core の hover/active 解除セマンティクス（`on_pointer_leave` / `on_pointer_cancel`）は `crates/core/tests/interaction.rs` の native `#[test]` で検証済み。
- しかし **実 DOM イベント → `web-sys` リスナー → 座標変換 → `pending_pointer` → drain → Core → `poll_events()`** という自己配線そのものは、ブラウザ無しでは一度も通らない。`#[cfg(target_arch = "wasm32")]` のクロージャ登録部分が静かに壊れても native テストは緑のまま。

## Decision

**自己配線した DOM 入力経路の回帰テストは、最も忠実度の高い地点（実 DOM イベント発火）で `#[wasm_bindgen_test]` を headless browser で実行して行う。**

- テストファイル: `crates/adapters/web/tests/pointer_input_browser.rs`（`#![cfg(target_arch = "wasm32")]`）。
- 実行: `wasm-pack test --headless --firefox crates/adapters/web -- --no-default-features --features backend-null --test pointer_input_browser`。
- **`backend-null` でビルドする**: WebGPU / EditContext を要求しないため、Firefox headless で WebGPU 無しに実行できる（Chrome/Chromium 依存を持ち込まない）。
- アサーション対象は **外部から観測可能な振る舞い**: 実 `PointerEvent` を canvas に dispatch し、`render()` で drain したのち `poll_events()` の delivery 行に `HoverEnter` / `HoverLeave` が現れることを assert する。hover 集合のメンバシップ等の内部状態は見ない。
- **test-only な wasm export は追加しない**（ADR-0072）。テストは既存の public bindings（`element_create` / `element_set_style` / `set_root` / `register_listener` / `render` / `poll_events`）だけで経路を駆動する。

CI には `.github/workflows/wasm-c3.yml` に Firefox headless ジョブ（`pointer-input-firefox`）を追加し、自己配線経路を回帰から保護する。

## Considered Options

- **native `#[test]` のみ**：`web-sys` クロージャ登録・実イベント dispatch・座標変換の結線を一切通らないため、自己配線のリグレッションを検出できない。却下。
- **Chrome/Chromium headless**：将来 WebGPU / EditContext を要するテストには必要だが、本経路は `backend-null` で足りるため CI 依存を増やす理由がない。本 ADR の範囲外。
- **ピクセル readback / golden-frame による revert の視覚検証**：GPU 依存で brittle。`poll_events()` の `HoverLeave` delivery を revert のシグナルとして採用する（ADR-0079 と同じ「構造化された観測可能シグナル」方針）。却下。

## Consequences

- 自己配線した Pointer Events 経路が、実ブラウザで end-to-end に一度通るようになり、`#[cfg(target_arch = "wasm32")]` のクロージャ部分のリグレッションが CI で検出される。
- Firefox headless + `backend-null` のため WebGPU/Chrome 依存を持ち込まず CI で安定実行できる。
- 新しい入力イベントを自己配線する際、同じハーネスに forced-DOM-event テストを追加する前例ができた。

## 関係

- ADR-0080：Platform Adapter が DOM イベントを自己配線する（本テストが守る対象）。
- ADR-0082：Pointer Events 統一 + `pointerleave`/`pointercancel` の hover/active 解除セマンティクス。
- ADR-0072：test-only wasm export を追加しない方針。
- ADR-0079：Canvas Mode のクロスシームテストは観測可能な構造化シグナルで行う前例。
