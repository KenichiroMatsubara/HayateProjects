# 擬似状態切替の補間アニメーションを Render Layer が担う（`transition` 語彙）

**Status: accepted（HITL — issue #209）**

**Date: 2026-06-13**

## Context

ADR-0056 で `:hover` / `:active` / `:focus` は Hayate CSS の一部として Render Layer が
effective style に合成するようになった。ただし切替は常に即時で、CSS の `transition` に
相当する補間がなかった。ADR-0086 は `render(timestamp_ms)` と `visual_dirty` による
dirty-gated なフレームループを、ADR-0032 はカーソル点滅という「時間駆動で `visual_dirty`
を立て続ける」前例を用意済みだった。新たなタイマー機構を足さず、この既存インフラの上に
補間を載せられる。

スコープは**擬似状態切替のみ**。`setStyle` 直接呼び出し（pseudo を経由しない mutation）は
従来どおり即時反映とする。

## Decision

1. **語彙追加（Protocol Contract）** — `style_tags.json` に `TRANSITION_DURATION`
   （`f32`、ミリ秒）と `TRANSITION_TIMING`（`enum:transition_timing` = `ease | linear |
   ease-in | ease-out | ease-in-out`）を追加する。両者は visual prop として `Visual` に
   解決される（layout/text には影響しない）。DOM 写像はそれぞれ CSS
   `transition-duration`（`ms` フォーマット）/ `transition-timing-function` に直接対応し、
   `dom_style_mapper.rs`（Rust HTML Mode）と `catalog.ts`（Tsubame DOM Renderer）の双方を
   spec から生成する（ADR-0070）。

2. **トリガは擬似状態切替の単一シーム** — 補間の開始は `mark_pseudo_activation_dirty` に
   集約する。対象要素の解決済み `transition-duration > 0` のとき、切替**前**の on-screen
   な effective visual を曲線の始点 `from` として捕捉し、`TransitionState` を登録する。
   全擬似状態で「state を変更する**前**にマークする」よう入力サーフェスを揃え
   （`:active` の press/up/cancel 経路を含む）、捕捉される `from` が常に切替前の見た目に
   なるようにする。

3. **`render(timestamp_ms)` が補間を進める** — カーソル点滅と同位置で
   `advance_transitions(timestamp_ms)` を呼ぶ。各 transition は最初の render で開始時刻を
   アンカーし、経過から線形 `raw ∈ [0,1]` を求め、`transition-timing` の cubic-bezier で
   eased progress に写す。進行中は対象を `visual_dirty`（SelfOnly）にマークしてフレーム
   ループを継続させ、完了フレームを最後にマークしてから `TransitionState` を破棄する。
   完了後は再マークされないのでループは静止する。

4. **補間は scene_build の解決直後に適用** — `resolve_effective` で得た target visual を
   `blend_transition` で `from→target` に補間してから lowering する。補間対象は連続値の
   `background-color` / `border-color` / `text-color` / `opacity` / `border-radius` /
   `border-width`。enum 系・離散値は補間せず target を即時採用する（補間不能なため）。
   incremental（`emit_element`）と full rebuild（`walk_ephemeral`）の双方で同じ blend を
   通し、paint parity（ADR-0086）を維持する。

## Consequences

- 新たなタイマー・アニメーションランタイムは持たない。補間は既存の `render` /
  `visual_dirty` / scene-lowering の上に乗る。
- `setStyle` 直接呼び出しは pseudo シームを通らないため、`transition-duration` の有無に
  かかわらず即時反映のまま。
- `transition-duration: 0`（または未指定）は即時切替で、ADR-0056 の従来挙動を維持する。
- 既知の単純化: 補間途中で逆方向の切替が起きた場合、`from` は切替時点の解決 visual を
  使う（途中値からの厳密な連続反転ではない）。受け入れ条件の範囲外であり、必要になれば
  別スライスで深める。
- DOM Renderer はブラウザの CSS `transition` に委譲するため、Canvas 経路（Render Layer
  補間）と DOM 経路で同じ語彙から二系統の補間が生成される。
