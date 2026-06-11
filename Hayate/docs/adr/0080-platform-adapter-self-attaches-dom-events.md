# Platform Adapter (hayate-adapter-web) が DOM イベント購読を自前で行う

**Status: accepted**

現状、`mousemove`/`mousedown`/`mouseup`/`wheel` の購読・座標変換は `Tsubame/packages/renderer-canvas/src/init.ts`（TS, Tsubame host）が担っており、resizeイベントの購読自体は存在せず host が手動で `IRenderer.resize()` を呼ぶ前提になっていた。これは CONTEXT.md の「`hayate-adapter-web` 等は input 変換と描画 flush のみ」という定義と乖離していた。

ブラウザの DOM が自動で担う入力・lifecycle イベント（pointer/wheel/resize/touch 等）は、host 側の glue コードを必要とせず `hayate-adapter-web`（Rust/WASM, `web-sys` 経由）が canvas/window に対して自前でイベントリスナーを登録し、座標変換・`on_resize` 呼び出し等まで完結させる。host/app は `element_set_scroll_offset`（SCR-02）等の既存プログラマティック API を通じて明示的に上書き・操作できるが、これはオプトインの追加経路であり、自動配線の代替ではない。

## Considered Options

- **現状維持（TS host が DOM イベントを購読し、wasm export を呼ぶ）**: 動作するが、resize 検知のような新規イベントを追加するたびに TS 側 glue の実装漏れが発生しやすく、「DOM が自動でやってくれること」を host ごとに再実装する負担が生じる。
- **Platform Adapter 自己配線（採用）**: `hayate-adapter-web` 初期化時に自前で `add_event_listener_with_callback` する。DOM が自動で提供する挙動は Hayate が自動で引き継ぎ、host は明示的なプログラマティック操作のみ追加で提供する。

## Consequences

- `init.ts` の `attachPointerInput` 相当のロジック（`mousemove`/`mousedown`/`mouseup`/`wheel` の購読・`toCanvas()` 座標変換）は `hayate-adapter-web` 初期化時の自前イベントリスナー登録に置き換わる。
- resize 検知（`ResizeObserver`）・Pointer Events 統一によるタッチ対応・Viewport Condition の resize 駆動再評価は、いずれもこの自動配線の上に構築される。
- Tsubame host の役割は `poll_events()` によるdispatch結果の受信とアプリ listener 実行、および明示的なプログラマティック API 呼び出しに純化される。
