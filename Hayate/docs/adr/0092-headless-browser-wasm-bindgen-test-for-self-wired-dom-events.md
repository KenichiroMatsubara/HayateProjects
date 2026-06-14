# 自己配線DOMイベントは headless browser の wasm-bindgen-test で検証する

**Status: accepted**

ADR-0080 により `hayate-adapter-web` が pointer / wheel / leave 等の DOM イベントを `web-sys` で自前配線するようになった。これらの thin wiring は従来のテスト戦略——純粋ロジックを native `#[test]`、ADR-0079 の golden-frame 整合を Node-vitest + WASM（`backend-null` / `backend-tiny-skia`）で検証——では到達できない領域にある。実際、Canvas モードで hover の `:hover` スタイルが解除されず残り続けるバグ（DOM モードはブラウザネイティブ `:hover` が leave を自動処理するため発生しない）は、「`pointerleave` が Core に伝搬し hover が解除される」配線そのものの欠落であり、Node-vitest（jsdom には実 DOM イベントループも実 canvas も無い）では忠実に再現・検証できなかった。

自己配線した DOM イベント経路は `wasm-pack test --headless --firefox` による `#[wasm_bindgen_test]` で検証する。検証は `poll_events()` 経由の delivery（`HoverLeave` 等、ADR-0018 / ADR-0034）を assert し、テスト専用 export は追加しない（ADR-0072「raw 層公開拒否」と整合）。

## Considered Options

- **Node-vitest + WASM ハーネスを拡張する**: 既存 C3 ハーネス（`Tsubame/packages/renderer-canvas` の `test:wasm`）を流用する案。だが jsdom には実 DOM イベントループ・実 canvas が無く、自己配線リスナーへの `pointerleave` dispatch を忠実に再現できない。配線欠落バグの回帰を捕捉できない。
- **headless browser の wasm-bindgen-test（採用）**: 実ブラウザ上で canvas に DOM イベントを dispatch し、`poll_events()` の delivery を assert する。自己配線層を実環境で初めて自動検証できる。
- **ピクセル読み戻し**: render 後にフレームバッファの色を読み視覚的復帰を検証する案。最高忠実度だが headless での実 WebGPU / canvas と pixel 配線が必要で重くフレーキー。

## Consequences

- `.github/workflows/wasm-c3.yml` に Firefox headless の job を追加する。テストは `--no-default-features --features backend-null` でビルドし、WebGPU / EditContext に非依存とする（`HayateElementRenderer::init` が WebGPU adapter を取得しないため Firefox headless で安定動作する。pointer / hit-test / `poll_events` は backend 非依存）。
- Chrome は WebGPU / EditContext を要する将来テストでのみ追加を検討する。`pointerdown/move/up/cancel` / `pointerleave` は標準 DOM であり Firefox で十分。
- 新規 CI 依存（geckodriver / Firefox）と実行時間が増えるが、ADR-0080 で Rust 側へ移した自己配線層の回帰を初めて自動的にロックできる。
