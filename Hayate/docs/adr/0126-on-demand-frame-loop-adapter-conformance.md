# Platform Front / renderer adapter は App Host の idle/wake 契約に従う（毎フレーム自己再スケジュール禁止）

**Status: proposed (draft)**

**Date: 2026-06-30**

> 2026-07-20: Android の Choreographer 駆動、wake coalescing、overload 時の latest-wins handoff は ADR-0154 で accepted decision として確定した。

## Context

ADR-0117 は App Host のフレームループを **`tick(timestamp_ms)` ＋ 構築時注入の `request_redraw()`** で定義し、**pending（進行中 transition / カーソル点滅 / スクロール物理 = `visual_dirty`）が無ければ idle に落ちる**（毎フレーム回し続けない）on-demand モデルを規定している。wake 源は三つ（継続・入力到着・非同期 signal 変化）で、入口は `request_redraw()` の一つ。

しかし実アダプタがこの契約を破っていることがコード分析で判明した：

- **Web**: `Tsubame/packages/renderer-hayate/src/hayate-renderer.ts` の `frame()` が末尾で **無条件に** `this.frameHandle = this.requestFrame(this.frame)` を呼び、`raw.render()` を毎フレーム実行する。dirty の有無を見ない。
- **Android**: `Hayate/crates/platform/mobile/android/src/app_tsubame.rs` のメインループが ~16ms ごとに **無条件に** `pump_frame()` ＋ `tree.render()` を回す（二重 render の指摘もある）。

結果、何も変化していない idle 時も 60–120Hz で render が走り、中位 SoC では GPU が休まず **サーマルスロットリング**と電池消費を招く。これは ADR-0125 のレイヤキャッシュ以前の、契約違反による無駄である。

## Decision

**フレームループの自己再スケジュールを撤廃し、全 Platform Front / renderer adapter を ADR-0117 の idle/wake 契約に従わせる。**

- `render()`（描画）と **次フレームのスケジューリングを分離**する。描画後に次フレームを要求するのは、App Host が「継続すべき pending がある」と判断したとき（`tick` 末尾の `request_redraw()`）に限る。
- idle（pending なし）では **フレームを 1 枚も出さない**。idle から起こすのは三つの wake 源（継続＝App Host、入力到着＝Platform Adapter、非同期 signal 変化＝consumer）だけ。
- **Web**: `hayate-renderer.ts` の `frame()` 末尾の無条件 `requestFrame` を撤廃。`requestFrame` は App Host/`raw` が「継続が必要」と返したときのみ呼ぶ。入力到着・signal 変化での冷間始動はアダプタ/consumer が `request_redraw()` を叩く。
- **Android**: `app_tsubame.rs` のループを「無条件 pump」から「`Choreographer` 駆動＋ wake 起動」へ。二重 render を解消し、`tick` 1 回 = 1 render とする。

## Considered Options

- **現状維持（毎フレーム pump）**: ADR-0117 違反のまま idle 描画で熱/電池を浪費。却下。
- **ADR-0117 を毎フレーム駆動に書き換える**: on-demand 設計そのものを捨てることになり、Blink/Flutter（共に on-demand）とも逆行。却下。

## Consequences

- idle 時の GPU/CPU 稼働がゼロになり、サーマルスロットリングと電池消費が改善する（ADR-0125 とは独立した即効性のある改善）。
- Phase 0 として最初に着手し、これ自体で Phone 3a のジャンク/発熱の相当部分が解消する見込み。以降の計測ベースラインを安定させる。
- カーソル点滅・transition・スクロール物理は「継続 pending」として正しく次フレームを要求し続ける（退行しないことをテストで保証）。

## 関係

- **enforces** ADR-0117（App Host boot seam: tick / request_redraw / idle）。
- ADR-0125 のロールアウト Phase 0。
- ADR-0080（入力 ingress は Platform Adapter 所有 = 入力 wake の出所）。
- ADR-0154（Android Latest-Wins Frame Scheduling）が本ADRのAndroid schedulingを具体化する。
