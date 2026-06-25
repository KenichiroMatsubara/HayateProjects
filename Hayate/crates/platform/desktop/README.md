# `platform/desktop/` — Desktop Platform Front（windowing leaf 着手・ADR-0118）

Desktop family（macos / windows / linux）の adapter 置き場。ADR-0118 で **最初の windowing leaf**
に着手し、winit single crate **`hayate-platform-desktop`** が native window を開いて vello/wgpu の
`Surface` 実装で描画結果を画面へ出す Platform Front を持つ。**capability facade（audio 等）は依然
0** のままで、空 facade を作らない規律（ADR-0117）は維持する。

grouping doctrine の正本は [`../README.md`](../README.md)。

## いま在るもの（`hayate-platform-desktop`）

- **windowing**: winit が macos / windows / linux を windowing / event-loop / GPU surface の層で
  1 crate に畳む（ADR-0118 — per-OS leaf 分割は native capability / native IME 着手まで遅延）。
- **Surface = vello / wgpu**: `WindowSurface` が vello の `render_to_texture` + `TextureBlitter` で
  winit の wgpu surface に blit する（`Backend = wgpu 唯一`・native primary の本番経路）。
- **フレーム駆動**: winit event loop が `App Host` を構築し、`request_redraw` を `window.request_redraw()`
  に配線、`RedrawRequested` で `tick(timestamp_ms)` を呼ぶ。継続フレーム判定は App Host が所有し、
  Platform Front はスケジューリングのみ（ADR-0117）。
- **表示**: 共有 demo fixture（`hayate_demo_fixtures::tasks_tree`）の "Tasks" UI を **静的 1 枚**で
  present する（issue #505 の tracer bullet）。consumer（Hayabusa / Tsubame Canvas Renderer）は
  mount しない（`DeliverySink` 無し）。
- **resize / HiDPI**: winit の物理サイズ・`scale_factor` を `ViewportMetrics::from_physical_size`
  に渡し、wgpu surface を再 configure しつつ `set_viewport` を反映する（ADR-0080）。

実行: `cargo run -p hayate-platform-desktop --bin hayate-desktop`（GPU アダプタが必要）。

## まだやらないこと（後続スライス）

- **入力**: pointer / keyboard / IME は未配線（別スライス・ADR-0118）。
- **per-OS leaf 分割**: windowing 層は winit が当面 collapse する。native capability（audio 等）/
  native IME（TSF / TSM / IBus）着手時に macos / windows / linux の leaf へ割る（ADR-0117/0118）。
- **capability trait / facade を先置きしない。** 契約（trait）の正本は常に **Core**（`Surface` /
  `ImeBridge` / `FontFetcher` と同型）。最初の desktop capability の trait は、その capability を実装
  する最初の leaf 着手時に Core へ追加する（空 trait / 空 facade を先置きしない）。
- **機構は借りない。** Flutter channel / RN bridge の機構（ランタイム dispatch）は借りない。
  借りるのは taxonomy（カタログ）だけ。
