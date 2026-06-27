# ビューポートのサイズ追従は CanvasRenderer の責務（ResizeObserver ＋ devicePixelRatio）

> **用語更新（ADR-0011・2026-06-27）**: 本 ADR の "Canvas Renderer" / `CanvasRenderer` / `@tsubame/renderer-canvas` は **Hayate Renderer** / `HayateRenderer` / `@tsubame/renderer-hayate` に改名された（タイトル含む）。なお本 ADR の resize 責務は ADR-0080（host/adapter が viewport を所有）で既に supersede 済み。本文は決定当時の記録として原文のまま。

**Status: accepted**

**Date: 2026-06-11**

## Context

`CanvasRenderer.resize()` は存在するが呼び出し元がなく、初期化時に `getBoundingClientRect()` を一度読むだけだった。ウィンドウリサイズで CSS がピクセルバッファを引き伸ばし、Canvas モードだけ歪んで再レイアウトされない。devicePixelRatio も未考慮で HiDPI では常時ぼける。DOM モードではブラウザが reflow を無償で行い、アプリコードにリサイズ処理は登場しない。

## Decision

ビューポートのサイズ追従は **CanvasRenderer が所有**する。レンダラーが自身の canvas に `ResizeObserver` を張り、CSS レイアウトサイズ × devicePixelRatio でピクセルバッファを同期し、Hayate へ resize を通知する。アプリコードはどちらのレンダラーでもサイズ管理を書かない（レンダラー間パリティ。ADR-0002 system-wide の体験原則をレイアウトサイズにも適用）。

- レイアウト座標は CSS px、描画は物理 px。Hayate 側の adapter 契約に scale（dpr）の受け渡しを追加する必要がある。
- 公開 `resize()` はテスト・特殊ホスト向けに残し、observer は options で opt-out 可能にする。

## Considered Options

- **ホスト/アプリ所有**（アプリが window resize を聴いて `resize()` を呼ぶ）: 埋め込み用途では柔軟だが、全アプリが書き忘れリスクを負い、DOM モードとの体験差が残る。opt-out + 公開 `resize()` で柔軟性は確保できるため却下。
